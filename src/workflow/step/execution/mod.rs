use camino::Utf8PathBuf as PathBuf;
use default::DefaultExecutor;
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
use crate::{nix_environment::NixRunCommand, workflow::CommandError};

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

    pub fn build_job(
        &self,
        command: &Box<dyn NixRunCommand>,
        log_file: File,
        step: StepInfo,
    ) -> Job {
        Job::new(self.execution_command(&command), log_file, step)
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

pub struct Job {
    command: Command,
    log_file: File,
    step: StepInfo,
}

impl Job {
    pub fn new(command: Command, log_file: File, step: StepInfo) -> Self {
        Self {
            command,
            log_file,
            step,
        }
    }

    pub fn execute(mut self) -> Result<RunningJob, ExecutionError> {
        let log_file_stderr = self
            .log_file
            .try_clone()
            .map_err(|err| ExecutionError::LogFileDuplication(self.step.log.clone(), err))?;

        let child = self
            .command
            .stdout(Stdio::from(self.log_file))
            .stderr(Stdio::from(log_file_stderr))
            .spawn()
            .map_err(|err| ExecutionError::Spawn(CommandError::new_io(&self.command, err)))?;

        let progress_scanner = self.step.progress_scanning.as_ref()
            .map(|scanning_info| ProgressScanner::new(scanning_info))
            .transpose()
            .map_err(|err| ExecutionError::ProgressScanSetup(err))?;

        Ok(RunningJob {
            child,
            command: self.command,
            progress_scanner,
            step: self.step,
        })
    }

    pub fn step_name(&self) -> &str {
        &self.step.name
    }
}

pub struct RunningJob {
    child: Child,
    command: Command,
    progress_scanner: Option<ProgressScanner>,
    step: StepInfo,
}

impl RunningJob {
    pub fn try_wait(&mut self) -> Result<bool, ExecutionError> {
        Ok(self
            .child
            .try_wait()
            .map_err(|err| ExecutionError::Wait(format!("{:?}", self.command), err))?
            .is_some())
    }

    pub fn wait(mut self) -> Result<(), ExecutionError> {
        match self
            .child
            .wait()
            .map_err(|err| ExecutionError::Wait(format!("{:?}", &self.command), err))?
            .code()
        {
            Some(0) => Ok(()),
            Some(code) => Err(ExecutionError::NonZeroExitCode(
                format!("{:?}", self.command),
                code,
            )),
            None => Err(ExecutionError::SignalTermination(format!(
                "{:?}",
                self.command
            ))),
        }
    }

    pub fn read_progress(&mut self) -> Result<Option<u32>, ExecutionError> {
        match &mut self.progress_scanner {
            None => Ok(None),
            Some(scan_info) => {
                let log_contents = std::fs::read_to_string(&self.step.log)
                    .map_err(|err| ExecutionError::ProgressLogRead(self.step.log.clone(), err))?;

                let progress = scan_info
                    .read_progress(log_contents)
                    .map_err(|err| ExecutionError::ProgressScan(self.step.log.clone(), err))?;

                Ok(Some(progress))
            }
        }
    }

    pub fn step_name(&self) -> &str {
        &self.step.name
    }

    pub fn progress_indicator_max(&self) -> Option<u32> {
        self.progress_scanner.as_ref().map(|scanner| scanner.indicator_max())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("bla")]
    Spawn(CommandError),

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
