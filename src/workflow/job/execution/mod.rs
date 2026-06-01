use camino::Utf8PathBuf as PathBuf;
use clap::ValueEnum;
use default::{DefaultExecutionCommand, DefaultExecutionOptions};
use derive_more::Display;
use serde::Deserialize;
use slurm::{SlurmExecutionCommand, options::SlurmExecutionOptions};
use std::{error::Error, fmt::Debug, rc::Rc};

use crate::nix_environment::NixRunCommand;

use super::JobError;

mod default;
mod slurm;

#[derive(Display, Default, Clone, Debug, ValueEnum)]
pub enum ExecutionMethod {
    #[default]
    #[display("default")]
    Default,
    #[display("slurm")]
    Slurm,
}

#[derive(Debug, Default, Deserialize)]
pub struct ExecutionOptions {
    default: DefaultExecutionOptions,
    slurm: Option<SlurmExecutionOptions>,
}

pub trait JobExecutionCommand: Debug {
    fn new(
        method: ExecutionMethod,
        target: &Box<dyn NixRunCommand>,
        log: PathBuf,
        options: ExecutionOptions,
    ) -> Box<dyn JobExecutionCommand>
    where
        Self: Sized,
    {
        match method {
            ExecutionMethod::Default => Box::new(DefaultExecutionCommand::new(
                target,
                log,
                options.default,
            )),
            ExecutionMethod::Slurm => Box::new(SlurmExecutionCommand::new(
                target,
                log,
                options
                    .slurm
                    .ok_or(JobError::UnprovidedExecutorUsage(method))?,
            )),
        }
    }

    fn spawn(self: Box<Self>) -> Result<Box<dyn JobExecutionChild>, JobExecutionError>;
}

pub trait JobExecutionChild: Debug {
    fn try_wait(&mut self) -> Result<bool, JobExecutionError>;
    fn wait(&mut self) -> Result<(), JobExecutionError>;
}

pub trait ExecutionError: Error {}

#[derive(Clone, Debug, Display)]
#[display("{}", self.0.to_string())]
pub struct JobExecutionError(Rc<dyn ExecutionError>);
impl Error for JobExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}

//#[derive(Debug)]
//pub enum JobExecutionOptions {
//    Default(DefaultExecutionOptions),
//    Slurm(SlurmExecutionOptions),
//}
//
//#[derive(Debug)]
//pub struct JobExecutionCommand {
//    command: Command,
//    log: Option<File>,
//    options: JobExecutionOptions,
//}
//
//impl JobExecutionCommand {
//    pub fn new(
//        method: ExecutionMethod,
//        target: &Box<dyn NixRunCommand>,
//        options: ExecutionOptions,
//    ) -> Result<Self, ExecutionError> {
//        Ok(match method {
//            ExecutionMethod::Default => slurm_execution_command(
//                target,
//                options
//                    .slurm
//                    .ok_or(ExecutionError::UnprovidedExecutorUsage(method))?,
//            ),
//            ExecutionMethod::Slurm => default_execution_command(target, options.default),
//        })
//    }
//
//    pub fn log(&mut self, file: File) {
//        self.log = Some(file);
//    }
//
//    pub fn spawn(mut self) -> Result<JobExecutionChild, ExecutionError> {
//        if let Some(log) = self.log {
//            let log_stderr = log
//                .try_clone()
//                .map_err(|err| ExecutionError::LogFileDuplication(err.into()))?;
//
//            self.command
//                .stdout(Stdio::from(log))
//                .stderr(Stdio::from(log_stderr))
//        } else {
//            self.command.stdout(Stdio::piped()).stderr(Stdio::piped())
//        };
//
//        let child = self
//            .command
//            .spawn()
//            .map_err(|err| ExecutionError::Spawn(format!("{:?}", self.command), err.into()))?;
//
//        Ok(JobExecutionChild::new(child, self.command, self.options))
//    }
//}
//
//pub enum JobExecutionChild {
//    Default(DefaultExecutionChild),
//    Slurm(SlurmExecutionChild),
//}
//impl JobExecutionChild {
//    pub fn new(child: Child, command: Command, options: JobExecutionOptions) -> Self {
//        match options {
//            JobExecutionOptions::Default(options) => {
//                JobExecutionChild::Default(DefaultExecutionChild::new(child, command, options))
//            }
//            JobExecutionOptions::Slurm(options) => {
//                JobExecutionChild::Slurm(SlurmExecutionChild::new(child, command, options))
//            }
//        }
//    }
//}
