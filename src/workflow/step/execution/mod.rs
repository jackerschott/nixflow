use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use default::DefaultExecutor;
use indicatif::{ProgressBar, ProgressStyle};
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

pub enum Job {
    Pending(PendingJob),
    Executing,
    Running(RunningJob),
    Finishing,
    Finished(FinishedJob),
}

impl Job {
    pub fn new(command: Command, step: StepInfo) -> Job {
        Job::Pending(PendingJob { command, step })
    }


    pub fn is_pending(&self) -> bool {
        match self {
            Job::Pending(_) => true,
            _ => false,
        }
    }

    pub fn is_running(&self) -> bool {
        match self {
            Job::Running(_) => true,
            _ => false,
        }
    }

    pub fn is_running_and(&self, predicate: impl Fn(&RunningJob) -> bool) -> bool {
        match self {
            Job::Running(running) => predicate(running),
            _ => false,
        }
    }

    pub fn is_finished(&self) -> bool {
        match self {
            Job::Finished(_) => true,
            _ => false,
        }
    }
}

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
        FinishedJob::new(self.step)
    }

    pub fn execute(mut self) -> Result<RunningJob, JobExecutionError> {
        let non_existing_inputs = self
            .non_existing_inputs()
            .attach_job_name(self.step_name())?;
        if !non_existing_inputs.is_empty() {
            return Err(ExecutionError::InputExistence(
                non_existing_inputs
                    .into_iter()
                    .map(|path| path.to_owned())
                    .collect(),
            ))
            .attach_job_name(self.step_name());
        }

        std::fs::create_dir_all(
            self.step
                .log
                .parent()
                .expect("expected log to be validated as a file path"),
        )
        .map_err(|err| ExecutionError::LogFileParentDirectoryCreation(self.step.log.clone(), err))
        .attach_job_name(self.step_name())?;
        let log_file = File::create(&self.step.log)
            .map_err(|err| ExecutionError::LogFileCreation(self.step.log.clone(), err))
            .attach_job_name(self.step_name())?;
        let log_file_stderr = log_file
            .try_clone()
            .map_err(|err| ExecutionError::LogFileDuplication(self.step.log.clone(), err))
            .attach_job_name(self.step_name())?;

        let child = self
            .command
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_stderr))
            .spawn()
            .map_err(|err| ExecutionError::Spawn(format!("{:?}", self.command), err))
            .attach_job_name(self.step_name())?;

        RunningJob::new(child, self.command, self.step)
    }

    pub fn step_name(&self) -> &str {
        &self.step.name
    }
}

pub struct ProgressHandler {
    scanner: Option<ProgressScanner>,
    bar: ProgressBar,
}

impl ProgressHandler {
    fn new<M: Into<String>>(progress_scanner: Option<ProgressScanner>, message: M) -> Self {
        Self {
            bar: match progress_scanner
                .as_ref()
                .map(|scanner| scanner.indicator_max())
            {
                Some(indicator_max) => ProgressBar::new(indicator_max as u64)
                    .with_style(
                        ProgressStyle::default_bar()
                            .template("[{bar:40.green/black}] {pos:>3}/{len:3} {msg}")
                            .expect("expected template string to be correct")
                            .progress_chars("-- "),
                    )
                    .with_message(message.into()),
                None => ProgressBar::new_spinner()
                    .with_style(ProgressStyle::default_spinner())
                    .with_message(message.into()),
            },
            scanner: progress_scanner,
        }
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

pub struct RunningJob {
    child: Child,
    command: Command,
    progress: ProgressHandler,
    step: StepInfo,
}

impl RunningJob {
    pub fn new(child: Child, command: Command, step: StepInfo) -> Result<Self, JobExecutionError> {
        let progress_scanner = step
            .progress_scanning
            .as_ref()
            .map(|scanning_info| ProgressScanner::new(scanning_info))
            .transpose()
            .map_err(|err| ExecutionError::ProgressScanSetup(err))
            .attach_job_name(step.name.clone())?;

        Ok(Self {
            child,
            command,
            progress: ProgressHandler::new(progress_scanner, step.name.clone()),
            step,
        })
    }

    pub fn done(&mut self) -> Result<bool, JobExecutionError> {
        Ok(self
            .child
            .try_wait()
            .map_err(|err| ExecutionError::Wait(format!("{:?}", self.command), err))
            .attach_job_name(self.step_name())?
            .is_some())
    }

    pub fn finish(mut self) -> Result<FinishedJob, JobExecutionError> {
        let finished_job = match self
            .child
            .wait()
            .map_err(|err| ExecutionError::Wait(format!("{:?}", &self.command), err))
            .attach_job_name(self.step_name())?
            .code()
        {
            Some(0) => Ok(FinishedJob::new(self.step)),
            Some(code) => Err(ExecutionError::NonZeroExitCode(
                format!("{:?}", self.command),
                code,
            ))
            .attach_job_name(self.step_name()),
            None => Err(ExecutionError::SignalTermination(format!(
                "{:?}",
                self.command
            )))
            .attach_job_name(self.step_name()),
        };

        self.progress.finish();

        return finished_job;
    }

    pub fn step_name(&self) -> &str {
        &self.step.name
    }

    pub fn update_progress(&mut self) -> Result<(), JobExecutionError> {
        self.progress
            .update(&self.step.log)
            .attach_job_name(self.step_name())
    }

    pub fn map_progress_bar(
        mut self,
        f: impl Fn(ProgressBar) -> ProgressBar,
    ) -> Self {
        self.progress.bar = f(self.progress.bar);
        self
    }
}

pub struct FinishedJob {
    step: StepInfo,
}

impl FinishedJob {
    pub fn new(step: StepInfo) -> Self {
        Self { step }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("failed to spawn `{0}`\n{1}")]
    Spawn(String, std::io::Error),

    #[error("failed to check for the existence of {0}\n{1}")]
    InputExistenceCheck(PathBuf, std::io::Error),

    #[error("the following")]
    InputExistence(Vec<PathBuf>),

    #[error("failed to check for the existence of {0}\n{1}")]
    OutputExistenceCheck(PathBuf, std::io::Error),

    #[error("failed to create the parent directory for the specified log file `{0}`\n{1}")]
    LogFileParentDirectoryCreation(PathBuf, std::io::Error),

    #[error("failed to create the log file `{0}`")]
    LogFileCreation(PathBuf, std::io::Error),

    #[error("failed to duplicate log file handle to `{0}`\n{1}")]
    LogFileDuplication(PathBuf, std::io::Error),

    #[error("failed to poll `{0}`\n{1}")]
    Wait(String, std::io::Error),

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

#[derive(Debug, thiserror::Error)]
#[error("failed to execute `{0}`\n{1}")]
pub struct JobExecutionError(String, ExecutionError);

trait AttachJobName<T> {
    fn attach_job_name<S: Into<String>>(self, name: S) -> Result<T, JobExecutionError>;
}

impl<T> AttachJobName<T> for Result<T, ExecutionError> {
    fn attach_job_name<S: Into<String>>(self, job_name: S) -> Result<T, JobExecutionError> {
        self.map_err(|err| JobExecutionError(job_name.into(), err))
    }
}
