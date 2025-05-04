use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use serde::Deserialize;
use serde_with::{serde_as, OneOrMany};
use std::{collections::HashMap, fs::File};

use crate::nix_environment::{FlakeOutput, FlakeSource, NixEnvironment};

use super::{
    executors::{ExecutionCommand, ExecutionHandle, Executor},
    scheduler::Scheduler,
    Error,
};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(transparent)]
struct InputList {
    #[serde_as(as = "OneOrMany<_>")]
    inputs: Vec<Input>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(transparent)]
struct OutputList {
    #[serde_as(as = "OneOrMany<_>")]
    #[allow(unused)]
    outputs: Vec<Output>,
}

#[derive(Debug, Deserialize)]
struct Input {
    #[allow(unused)]
    path: PathBuf,

    #[serde(rename = "parentStep")]
    parent_step: Step,
}
#[derive(Debug, Deserialize)]
#[serde(transparent)]
struct Output {
    #[allow(unused)]
    path: PathBuf,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct Step {
    pub name: String,

    #[serde(default)]
    inputs: HashMap<String, InputList>,

    #[allow(unused)]
    outputs: HashMap<String, OutputList>,

    #[serde(default)]
    executor: Executor,

    log: PathBuf,

    progress: ProgressRetreivalInfo,

    #[serde(rename = "run")]
    #[allow(unused)]
    run_binary_path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct ProgressRetreivalInfo {
    #[serde(rename = "indicatorMax")]
    indicator_max: f32,

    #[serde(rename = "indicatorRegex")]
    indicator_regex: String,
}

impl Step {
    pub fn schedule<'s>(
        &'s self,
        scheduler: &mut Scheduler<'s>,
        nix_environment: &Box<dyn NixEnvironment>,
        flake_path: &Path,
    ) -> Result<(), Error> {
        for (_, input_list) in self.inputs.iter() {
            for input in input_list.inputs.iter() {
                input.parent_step.schedule(scheduler, nix_environment, flake_path)?;
            }
        }

        let run_command = nix_environment.run_command(
            FlakeOutput::new(FlakeSource::Path(flake_path.to_owned()), self.name.clone()),
            true,
        );

        std::fs::create_dir_all(
            self.log
                .parent()
                .expect("expected log to be validated as a file path"),
        )
        .map_err(|io_error| Error::IOSetupFailure {
            step_name: self.name.clone(),
            io_error,
        })?;
        let log_file = File::create(&self.log).map_err(|io_error| Error::IOSetupFailure {
            step_name: self.name.clone(),
            io_error,
        })?;

        scheduler.schedule(Job::new(
            self,
            self.executor.execution_command(&self, &run_command),
            log_file,
        ));

        Ok(())
    }
}

pub struct Job<'j> {
    command: ExecutionCommand<'j>,
    log_file: File,
    step: &'j Step,
}

impl<'j> Job<'j> {
    pub fn new(step: &'j Step, command: ExecutionCommand<'j>, log_file: File) -> Self {
        Self {
            command,
            log_file,
            step,
        }
    }

    pub fn execute(self) -> Result<ExecutionHandle<'j>, Error> {
        let log_file_stderr =
            self.log_file
                .try_clone()
                .map_err(|io_error| Error::IOSetupFailure {
                    step_name: self.step.name.clone(),
                    io_error,
                })?;

        self.command
            .run(self.log_file, log_file_stderr)
            .map_err(|execution_error| Error::StepExecutionFailure {
                step_name: self.step.name.clone(),
                execution_error,
            })
    }
}
