use camino::Utf8PathBuf as PathBuf;
use serde::Deserialize;
use serde_with::{serde_as, OneOrMany};
use std::collections::HashMap;

pub mod execution;
pub mod progress;

use execution::Executor;
use progress::ProgressScanningInfo;

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct InputList {
    #[serde_as(as = "OneOrMany<_>")]
    pub inputs: Vec<Input>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct OutputList {
    #[serde_as(as = "OneOrMany<_>")]
    pub outputs: Vec<Output>,
}

#[derive(Debug, Deserialize)]
pub struct Input {
    pub path: PathBuf,

    #[serde(rename = "parentStep")]
    pub parent_step: Step,
}
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct Output {
    pub path: PathBuf,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct Step {
    pub name: String,

    #[serde(default)]
    #[serde(rename = "inputs")]
    pub inputs: HashMap<String, InputList>,

    pub outputs: HashMap<String, OutputList>,

    #[serde(default)]
    pub executor: Executor,

    pub log: PathBuf,

    #[serde(rename = "progress")]
    pub progress_scanning: Option<ProgressScanningInfo>,

    #[serde(rename = "run")]
    #[allow(unused)]
    run_binary_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct StepInfo {
    pub name: String,
    inputs: Vec<PathBuf>,
    outputs: Vec<PathBuf>,
    log: PathBuf,
    progress_scanning: Option<ProgressScanningInfo>,
}

impl StepInfo {
    pub fn new(
        name: String,
        inputs: Vec<PathBuf>,
        outputs: Vec<PathBuf>,
        log: PathBuf,
        progress_scanning: Option<ProgressScanningInfo>,
    ) -> Self {
        Self {
            name,
            inputs,
            outputs,
            log,
            progress_scanning,
        }
    }
}

impl Into<StepInfo> for &StepInfo {
    fn into(self) -> StepInfo {
        self.clone()
    }
}
