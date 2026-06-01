use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use options::{FormatSlurmTime, MemorySize, SlurmExecutionOptions};
use state::JobState;
use std::{
    io::Write,
    process::{Command, Stdio},
    rc::Rc,
    str::FromStr,
};
use tempfile::NamedTempFile;

use crate::{
    commands::{AsCommandError, CommandError, OutputUtf8},
    nix_environment::NixRunCommand,
    utils::IoError,
};

use super::{ExecutionError, JobExecutionChild, JobExecutionCommand, JobExecutionError};

pub(super) mod options;
mod state;

pub type SlurmJobID = u64;

#[derive(Debug)]
pub(super) struct SlurmExecutionCommand {
    shell_command: String,
    log: PathBuf,
    options: SlurmExecutionOptions,
}
impl SlurmExecutionCommand {
    pub fn new(
        target: &Box<dyn NixRunCommand>,
        log: PathBuf,
        options: SlurmExecutionOptions,
    ) -> Self {
        Self {
            shell_command: target.shell_command(),
            log,
            options,
        }
    }
}

impl JobExecutionCommand for SlurmExecutionCommand {
    fn spawn(self: Box<Self>) -> Result<Box<dyn JobExecutionChild>, JobExecutionError> {
        let jd = slurm_execute(self.shell_command, &self.log, &self.options)?;

        Ok(Box::new(SlurmExecutionChild::new(job_id)))
    }
}

#[derive(Debug)]
pub struct SlurmExecutionChild {
    job_id: SlurmJobID,
}
impl JobExecutionChild for SlurmExecutionChild {}

impl SlurmExecutionChild {
    pub fn new(job_id: SlurmJobID) -> Self {
        SlurmExecutionChild { job_id }
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum SlurmError {
    #[error("failed poll the slurm job state\n{0}")]
    JobStatePoll(CommandError),

    #[error("failed to parse job state output from `{command}`\n{error}")]
    JobStateParsing { command: String, error: String },

    #[error("failed to create the sbatch script as a temporary file\n{0}")]
    JobExecutionScriptCreation(IoError),

    #[error("failed to write to the temporarily created sbatch script\n{0}")]
    JobExecutionScriptWrite(IoError),

    #[error("failed to execute the slurm job\n{0}")]
    JobExecute(CommandError),

    #[error("failed to read the slurm job ID from the output of `{command}`\n{error}")]
    JobExecutionReadJobID { command: String, error: String },
}
impl ExecutionError for SlurmError {}
impl From<SlurmError> for JobExecutionError {
    fn from(error: SlurmError) -> Self {
        JobExecutionError(Rc::new(error))
    }
}

pub fn slurm_execute(
    shell_command: String,
    log: &Path,
    options: &SlurmExecutionOptions,
) -> Result<SlurmJobID, SlurmError> {
    let mut command = Command::new("sbatch");
    command.arg("--account").arg(&options.account);

    if let Some(service_quality) = &options.quality_of_service {
        command.arg("--qos").arg(service_quality);
    }

    if let Some(constraint) = &options.constraint {
        command.arg("--constraint").arg(constraint);
    }

    if let Some(partitions) = &options.partitions {
        command.arg("--partition").arg(partitions.join(","));
    }

    command
        .arg("--time")
        .arg(options.runtime.format_slurm_time())
        .arg(match options.memory_size {
            MemorySize::AllAvailable => "--mem=0".to_owned(),
            MemorySize::Fixed((size, unit)) => {
                format!("--mem={size}{unit}", unit = unit.as_slurm_suffix())
            }
        })
        .arg("--cpus-per-task")
        .arg(options.cpu_count.to_string())
        .arg("--gpus")
        .arg(options.gpu_count.to_string());

    let mut execution_script =
        NamedTempFile::new().map_err(|err| SlurmError::JobExecutionScriptCreation(err.into()))?;
    write!(execution_script, "#!/bin/sh\n{shell_command}")
        .map_err(|err| SlurmError::JobExecutionScriptWrite(err.into()))?;

    command
        .arg(format!("--output={log}"))
        .arg(execution_script.into_temp_path());

    let output: OutputUtf8 = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| CommandError::new_io(&command, err))
        .map_err(|err| SlurmError::JobExecute(err))?
        .into();

    output
        .status
        .as_piped_command_result(&command, &output.stdout, &output.stderr)
        .map_err(|err| SlurmError::JobStatePoll(err))?;
    assert!(output.stderr.trim().is_empty());

    let output = output.stdout.trim();
    let job_id = output
        .strip_prefix(output)
        .ok_or("expected output to start with `Submitted batch job `".to_owned())
        .map_err(|error| SlurmError::JobExecutionReadJobID {
            command: format!("{command:?}"),
            error,
        })?;

    Ok(
        SlurmJobID::from_str(job_id).map_err(|err| SlurmError::JobExecutionReadJobID {
            command: format!("{command:?}"),
            error: format!(
                "failed to parse string after `Submitted batch job ` as an integer\n{err}"
            ),
        })?,
    )
}

pub fn poll_job_state(job_id: SlurmJobID) -> Result<JobState, SlurmError> {
    let mut command = Command::new("squeue");
    command
        .arg("--job")
        .arg(format!("{job_id}"))
        .arg("--format=%i %T %r");

    let output: OutputUtf8 = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| CommandError::new_io(&command, err))
        .map_err(|err| SlurmError::JobStatePoll(err))?
        .into();

    output
        .status
        .as_piped_command_result(&command, &output.stdout, &output.stderr)
        .map_err(|err| SlurmError::JobStatePoll(err))?;
    assert!(output.stderr.trim().is_empty());

    JobState::from_polling_output(&output.stdout).map_err(|error| SlurmError::JobStateParsing {
        command: format!("{command:?}"),
        error,
    })
}
