use camino::Utf8PathBuf as PathBuf;
use default::DefaultExecutor;
use serde::Deserialize;
use slurm::SlurmExecutor;
use std::{
    fs::File,
    process::{Child, Command, Stdio},
};

use super::{step::Step, CommandError};
use crate::nix_environment::NixRunCommand;

mod default;
mod slurm;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("execution failure\n{0}")]
    ExecutionFailure(CommandError),
}

pub struct ExecutionCommand<'c> {
    command: Command,
    step: &'c Step,
}

impl<'c> ExecutionCommand<'c> {
    pub fn run(
        mut self,
        stdout_log_file: File,
        stderr_log_file: File,
    ) -> Result<ExecutionHandle<'c>, Error> {
        let child = self
            .command
            .stdout(Stdio::from(stdout_log_file))
            .stderr(Stdio::from(stderr_log_file))
            .spawn()
            .map_err(|err| Error::ExecutionFailure(CommandError::new_io(&self.command, err)))?;

        Ok(ExecutionHandle {
            child,
            command: self.command,
            step: self.step,
        })
    }
}

pub struct ExecutionHandle<'h> {
    child: Child,
    command: Command,
    pub step: &'h Step,
}

impl ExecutionHandle<'_> {
    pub fn try_wait(&mut self) -> Result<bool, Error> {
        Ok(self
            .child
            .try_wait()
            .map_err(|io_error| {
                Error::ExecutionFailure(CommandError::new_io(&self.command, io_error))
            })?
            .is_some())
    }

    pub fn wait(mut self) -> Result<(), Error> {
        match self
            .child
            .wait()
            .map_err(|io_error| {
                Error::ExecutionFailure(CommandError::new_io(&self.command, io_error))
            })?
            .code()
        {
            Some(0) => Ok(()),
            Some(code) => Err(Error::ExecutionFailure(
                CommandError::new_non_zero_exit_code(&self.command, code),
            )),
            None => Err(Error::ExecutionFailure(
                CommandError::new_signal_termination(&self.command),
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
pub enum Executor {
    Default(DefaultExecutor),
    Slurm(SlurmExecutor),
}

impl Executor {
    pub fn execution_command<'s>(
        &'s self,
        step: &'s Step,
        target: &Box<dyn NixRunCommand>,
    ) -> ExecutionCommand {
        match self {
            Executor::Default(default) => default.execution_command(step, target),
            Executor::Slurm(slurm) => slurm.execution_command(step, target),
        }
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
