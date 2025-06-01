use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use default::DefaultExecutor;
use indicatif::ProgressBar;
use miette::Diagnostic;
use serde::Deserialize;
use slurm::SlurmExecutor;
use std::{
    fs::File,
    process::{Child, Command, Stdio},
};

use super::{
    progress::{ProgressScanError, ProgressScanner},
    StepInfo,
};
use crate::nix_environment::NixRunCommand;

mod default;
mod slurm;

#[derive(Debug, Deserialize)]
pub enum Executor {
    Default(DefaultExecutor),
    Slurm(SlurmExecutor),
}

impl Executor {
    fn execution_command<'s>(&'s self, target: &Box<dyn NixRunCommand>) -> Command {
        match self {
            Executor::Default(default) => default.execution_command(target),
            Executor::Slurm(slurm) => slurm.execution_command(target),
        }
    }

    pub fn build_job(&self, command: &Box<dyn NixRunCommand>, step: StepInfo) -> Job {
        Job::new(self.execution_command(&command), step)
    }
}

impl Default for Executor {
    fn default() -> Self {
        Executor::Default(DefaultExecutor {})
    }
}

impl std::fmt::Display for Executor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Executor::Default(default) => write!(f, "{default}"),
            Executor::Slurm(slurm) => write!(f, "{slurm}"),
        }
    }
}

#[derive(Debug)]
pub enum Job {
    Pending(PendingJob),
    Executing,
    Running(RunningJob),
    Finishing,
    #[allow(unused)]
    Finished(FinishedJob),
}

impl Job {
    pub fn new(command: Command, step: StepInfo) -> Job {
        Job::Pending(PendingJob::new(command, step))
    }

    pub fn update(&mut self) -> Result<(), JobExecutionError> {
        match self {
            Job::Running(running) => running.update_progress(),
            _ => Ok(()),
        }
    }

    pub fn is_finished(&self) -> bool {
        match self {
            Job::Finished(_) => true,
            _ => false,
        }
    }

    pub fn failed(&self) -> bool {
        match self {
            Job::Finished(finished) => finished.failed(),
            _ => false,
        }
    }

    pub fn failed_updates(&self) -> u32 {
        match self {
            Job::Running(running) => running.failed_updates,
            _ => 0,
        }
    }
}

#[derive(Debug)]
pub struct PendingJob {
    command: Command,
    step: StepInfo,
}

impl PendingJob {
    pub fn new(command: Command, step: StepInfo) -> Self {
        Self { command, step }
    }

    pub fn non_existing_associated_paths<'p>(
        &self,
        paths: &'p Vec<PathBuf>,
    ) -> Result<Vec<&'p Path>, (PathBuf, std::io::Error)> {
        paths
            .iter()
            .filter_map(|output| match std::fs::exists(output) {
                Ok(exists) => (!exists).then_some(Ok(output.as_path())),
                Err(err) => Some(Err((output.to_owned(), err))),
            })
            .collect()
    }

    fn non_existing_inputs(&self) -> Result<Vec<&Path>, ExecutionError> {
        self.non_existing_associated_paths(&self.step.inputs)
            .map_err(|(path, err)| ExecutionError::InputExistenceCheck(path, err))
    }

    fn non_existing_outputs(&self) -> Result<Vec<&Path>, ExecutionError> {
        self.non_existing_associated_paths(&self.step.outputs)
            .map_err(|(path, err)| ExecutionError::OutputExistenceCheck(path, err))
    }

    pub fn finish(self) -> FinishedJob {
        assert!(self
            .non_existing_outputs()
            .is_ok_and(|outputs| outputs.is_empty()));
        FinishedJob::new_success(self.step)
    }

    pub fn terminate(self) -> FinishedJob {
        FinishedJob::new_terminated(self.step)
    }

    pub fn execute(mut self) -> Result<RunningJob, JobExecutionError> {
        let non_existing_inputs = self.non_existing_inputs().attach_step_info(&self.step)?;
        if !non_existing_inputs.is_empty() {
            return Err(ExecutionError::InputExistence {
                input_paths: non_existing_inputs
                    .into_iter()
                    .map(|path| path.to_owned())
                    .collect(),
            })
            .attach_step_info(self.step);
        }

        let non_existing_outputs = self.non_existing_outputs().attach_step_info(&self.step)?;
        if non_existing_outputs.is_empty() {
            let step = self.step.clone();
            return Err(ExecutionError::ShouldDirectlyFinish(self)).attach_step_info(step);
        }

        std::fs::create_dir_all(
            self.step
                .log
                .parent()
                .expect("expected log to be validated as a file path"),
        )
        .map_err(|err| ExecutionError::LogFileParentDirectoryCreation(self.step.log.clone(), err))
        .attach_step_info(&self.step)?;
        let log_file = File::create(&self.step.log)
            .map_err(|err| ExecutionError::LogFileCreation(self.step.log.clone(), err))
            .attach_step_info(&self.step)?;
        let log_file_stderr = log_file
            .try_clone()
            .map_err(|err| ExecutionError::LogFileDuplication(self.step.log.clone(), err))
            .attach_step_info(&self.step)?;

        let child = self
            .command
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_stderr))
            .spawn()
            .map_err(|err| ExecutionError::Spawn(format!("{:?}", self.command), err))
            .attach_step_info(&self.step)?;

        Ok(RunningJob::new(child, self.command, self.step))
    }

    pub fn step_name(&self) -> &str {
        &self.step.name
    }
}

#[derive(Debug)]
pub struct ProgressHandler {
    scanner: Option<ProgressScanner>,
    bar: ProgressBar,
}

impl ProgressHandler {
    fn new(scanner: Option<ProgressScanner>, bar: ProgressBar) -> Self {
        Self { bar, scanner }
    }

    fn update<P: AsRef<Path>>(&mut self, log: &P) -> Result<(), ExecutionError> {
        match &mut self.scanner {
            None => self.bar.tick(),
            Some(scan_info) => {
                let log_contents = std::fs::read_to_string(log.as_ref())
                    .map_err(|err| ExecutionError::ProgressLogRead(log.as_ref().to_owned(), err))?;

                let progress = scan_info
                    .read_progress(log_contents)
                    .map_err(|err| ExecutionError::ProgressScan(log.as_ref().to_owned(), err))?;

                self.bar.set_position(progress as u64);
            }
        }

        Ok(())
    }

    fn finish(&self) {
        self.bar.finish();
    }
}

#[derive(Debug)]
pub struct RunningJob {
    child: Child,
    command: Command,
    progress: Option<ProgressHandler>,
    step: StepInfo,
    failed_done_polls: u32,
    failed_updates: u32,
}

impl RunningJob {
    pub fn new(child: Child, command: Command, step: StepInfo) -> Self {
        Self {
            child,
            command,
            progress: None,
            step,
            failed_done_polls: 0,
            failed_updates: 0,
        }
    }

    pub fn progress_max(&self) -> Option<u32> {
        self.step
            .progress_scanning
            .as_ref()
            .map(|info| info.indicator_max)
    }

    pub fn with_progress(
        mut self,
        build_progress: impl Fn(&Self) -> ProgressBar,
    ) -> Result<Self, JobExecutionError> {
        let progress_scanner = self
            .step
            .progress_scanning
            .as_ref()
            .map(|scanning_info| ProgressScanner::new(scanning_info))
            .transpose()
            .map_err(|err| ExecutionError::ProgressScanSetup(err))
            .attach_step_info(&self.step)?;

        self.progress = Some(ProgressHandler::new(
            progress_scanner,
            build_progress(&self),
        ));

        Ok(self)
    }

    pub fn done(&mut self) -> Result<bool, JobExecutionError> {
        let result = self
            .child
            .try_wait()
            .map_err(|err| ExecutionError::Wait(format!("{:?}", self.command), err))
            .attach_step_info(&self.step);
        if result.is_err() {
            self.failed_done_polls += 1;
        }

        Ok(result?.is_some())
    }

    pub fn finish(mut self) -> Result<FinishedJob, JobExecutionError> {
        let finished_job = match self
            .child
            .wait()
            .map_err(|err| ExecutionError::Wait(format!("{:?}", &self.command), err))
            .attach_step_info(&self.step)?
            .code()
        {
            Some(0) => FinishedJob::new_success(self.step),
            Some(code) => FinishedJob::new_failure(
                self.step,
                ExecutionError::NonZeroExitCode(format!("{:?}", self.command), code),
            ),
            None => FinishedJob::new_failure(
                self.step,
                ExecutionError::SignalTermination(format!("{:?}", self.command)),
            ),
        };

        self.progress.inspect(|progress| progress.finish());

        return Ok(finished_job);
    }

    pub fn terminate(mut self) -> Result<FinishedJob, JobExecutionError> {
        self.child
            .kill()
            .map_err(|err| ExecutionError::Kill(format!("{:?}", &self.command), err))
            .attach_step_info(&self.step)?;

        self.progress.inspect(|progress| progress.finish());

        return Ok(FinishedJob::new_success(self.step));
    }

    pub fn step_name(&self) -> &str {
        &self.step.name
    }

    pub fn update_progress(&mut self) -> Result<(), JobExecutionError> {
        let result = match &mut self.progress {
            Some(progress) => progress.update(&self.step.log).attach_step_info(&self.step),
            None => Ok(()),
        };

        if result.is_err() {
            self.failed_updates += 1;
        }

        return result;
    }
}

#[derive(Debug)]
pub enum FinishedJob {
    Success {
        step: StepInfo,
    },
    Failure {
        error: ExecutionError,
        step: StepInfo,
    },
    Terminated {
        step: StepInfo,
    },
}

impl FinishedJob {
    pub fn new_success(step: StepInfo) -> Self {
        Self::Success { step }
    }

    pub fn new_failure(step: StepInfo, error: ExecutionError) -> Self {
        Self::Failure { step, error }
    }

    pub fn new_terminated(step: StepInfo) -> Self {
        Self::Terminated { step }
    }

    pub fn failed(&self) -> bool {
        matches!(self, FinishedJob::Failure { .. })
    }

    pub fn success(&self) -> bool {
        matches!(self, FinishedJob::Success { .. })
    }

    pub fn terminated(&self) -> bool {
        matches!(self, FinishedJob::Terminated { .. })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("failed to spawn `{0}`\n{1}")]
    Spawn(String, std::io::Error),

    #[error("failed to check for the existence of {0}\n{1}")]
    InputExistenceCheck(PathBuf, std::io::Error),

    #[error("the following inputs do not exist:\n\t{}", input_paths.iter().map(|path| format!("`{path}`")).collect::<Vec<_>>().join("\n\t"))]
    InputExistence { input_paths: Vec<PathBuf> },

    #[error("failed to check for the existence of {0}\n{1}")]
    OutputExistenceCheck(PathBuf, std::io::Error),

    #[error("the job should be finished directly")]
    ShouldDirectlyFinish(PendingJob),

    #[error("failed to create the parent directory for the specified log file `{0}`\n{1}")]
    LogFileParentDirectoryCreation(PathBuf, std::io::Error),

    #[error("failed to create the log file `{0}`")]
    LogFileCreation(PathBuf, std::io::Error),

    #[error("failed to duplicate log file handle to `{0}`\n{1}")]
    LogFileDuplication(PathBuf, std::io::Error),

    #[error("failed to poll `{0}`\n{1}")]
    Wait(String, std::io::Error),

    #[error("failed to kill `{0}`\n{1}")]
    Kill(String, std::io::Error),

    #[error("failed to execute `{0}`, terminated by a signal")]
    SignalTermination(String),

    #[error("failed to execute `{0}`, exit code {1} is non-zero")]
    NonZeroExitCode(String, i32),

    #[error("failed to setup progress scanning\n{0}")]
    ProgressScanSetup(ProgressScanError),

    #[error("failed to read progress from `{0}`\n{1}")]
    ProgressLogRead(PathBuf, std::io::Error),

    #[error("failed to read progress from `{0}`\n{1}")]
    ProgressScan(PathBuf, ProgressScanError),
}

#[derive(Debug, thiserror::Error, Diagnostic)]
#[error(
    "failed to execute `{name}`\n\
    {error}\n\
    check {log} or execute nixflow with `--inspect {name}` to inspect the job output",
    name = step.name,
    log = step.log,
)]
#[diagnostic(help("try doing this instead"))]
pub struct JobExecutionError {
    pub step: StepInfo,
    pub error: ExecutionError,
}

pub trait AttachStepInfo<T> {
    fn attach_step_info<S: Into<StepInfo>>(self, step: S) -> Result<T, JobExecutionError>;
}

impl<T> AttachStepInfo<T> for Result<T, ExecutionError> {
    fn attach_step_info<S: Into<StepInfo>>(self, step: S) -> Result<T, JobExecutionError> {
        self.map_err(|error| JobExecutionError {
            step: step.into(),
            error,
        })
    }
}
