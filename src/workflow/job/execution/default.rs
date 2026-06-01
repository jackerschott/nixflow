use camino::Utf8PathBuf as PathBuf;
use std::{
    fs::File,
    process::{Command, Stdio},
    rc::Rc,
};

use serde::Deserialize;

use crate::{commands::clone_command, nix_environment::NixRunCommand, utils::IoError};

use super::{ExecutionError, JobExecutionChild, JobExecutionCommand, JobExecutionError};

#[derive(Debug, Default, Deserialize)]
pub struct DefaultExecutionOptions {}

#[derive(Debug)]
pub struct DefaultExecutionCommand {
    command: Command,
    log: PathBuf,
}
impl DefaultExecutionCommand {
    pub fn new(
        target: &Box<dyn NixRunCommand>,
        log: PathBuf,
        _default: DefaultExecutionOptions,
    ) -> Self {
        Self {
            command: clone_command(
                target
                    .command()
                    .unwrap_or(Command::new("bash").arg("-c").arg(target.shell_command())),
            ),
            log,
        }
    }
}
impl JobExecutionCommand for DefaultExecutionCommand {
    fn spawn(mut self: Box<Self>) -> Result<Box<dyn JobExecutionChild>, JobExecutionError> {
        let log_file = File::create(&self.log)
            .map_err(|err| DefaultExecutionError::LogFileCreation(self.log.clone(), err.into()))?;
        let log_file_stderr = log_file
            .try_clone()
            .map_err(|err| DefaultExecutionError::LogFileDuplication(err.into()))?;

        let child = self
            .command
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_stderr))
            .spawn()
            .map_err(|err| {
                DefaultExecutionError::Spawn(format!("{:?}", self.command), err.into())
            })?;

        Ok(Box::new(DefaultExecutionChild::new(child)))
    }
}

#[derive(Debug)]
pub struct DefaultExecutionChild {
    child: std::process::Child,
}
impl DefaultExecutionChild {
    pub fn new(child: std::process::Child) -> Self {
        DefaultExecutionChild { child }
    }
}
impl JobExecutionChild for DefaultExecutionChild {}

#[derive(Clone, Debug, thiserror::Error)]
enum DefaultExecutionError {
    #[error("failed to create the log file `{0}`\n{1}")]
    LogFileCreation(PathBuf, IoError),

    #[error("failed to duplicate log file handle\n{0}")]
    LogFileDuplication(IoError),

    #[error("failed to spawn `{0}`\n{1}")]
    Spawn(String, IoError),
}
impl ExecutionError for DefaultExecutionError {}
impl From<DefaultExecutionError> for JobExecutionError {
    fn from(error: DefaultExecutionError) -> Self {
        JobExecutionError(Rc::new(error))
    }
}
