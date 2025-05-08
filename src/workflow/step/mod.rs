use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use serde::Deserialize;
use serde_with::{serde_as, OneOrMany};
use std::{collections::HashMap, fs::File};

use crate::nix_environment::{FlakeOutput, FlakeSource, NixEnvironment};

pub mod execution;
pub mod progress;

use super::{scheduler::Scheduler, WorkflowError};
use execution::Executor;
use progress::ProgressScanningInfo;

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

    pub log: PathBuf,

    #[serde(rename = "progress")]
    pub progress_scanning: Option<ProgressScanningInfo>,

    #[serde(rename = "run")]
    #[allow(unused)]
    run_binary_path: PathBuf,
}

impl Step {
    pub fn schedule<'s>(
        self,
        scheduler: &mut Scheduler,
        nix_environment: &Box<dyn NixEnvironment>,
        flake_path: &Path,
    ) -> Result<(), WorkflowError> {
        for (_, input_list) in self.inputs.into_iter() {
            for input in input_list.inputs.into_iter() {
                input
                    .parent_step
                    .schedule(scheduler, nix_environment, flake_path)?;
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
        .map_err(|io_error| WorkflowError::IOSetupFailure {
            step_name: self.name.clone(),
            io_error,
        })?;
        let log_file =
            File::create(&self.log).map_err(|io_error| WorkflowError::IOSetupFailure {
                step_name: self.name.clone(),
                io_error,
            })?;

        let step_info = StepInfo::new(self.name, self.log, self.progress_scanning);
        scheduler.schedule(self.executor.build_job(&run_command, log_file, step_info));

        Ok(())
    }
}

pub struct StepInfo {
    name: String,
    log: PathBuf,
    progress_scanning: Option<ProgressScanningInfo>,
}

impl StepInfo {
    pub fn new(
        name: String,
        log: PathBuf,
        progress_scanning: Option<ProgressScanningInfo>,
    ) -> Self {
        Self {
            name,
            log,
            progress_scanning,
        }
    }
}
