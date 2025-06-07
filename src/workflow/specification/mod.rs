use std::collections::HashMap;

use camino::Utf8PathBuf as PathBuf;
use miette::{Context, IntoDiagnostic};
use parsing::{TargetList, WithSourceIndication};
use serde::Deserialize;
use serde_json::Value;
use serde_with::{serde_as, OneOrMany};

use progress::ProgressScanningInfo;

use super::job::execution::Executor;

mod parsing;
pub mod progress;

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
impl Step {
    pub fn info(&self) -> StepInfo {
        StepInfo::new(
            self.name.clone(),
            self.inputs
                .values()
                .flat_map(|input_list| input_list.inputs.iter().map(|input| input.path.clone()))
                .collect(),
            self.outputs
                .values()
                .flat_map(|output_list| output_list.outputs.iter().map(|output| output.path.clone()))
                .collect(),
            self.log.clone(),
            self.progress_scanning.clone(),
        )
    }
}

#[derive(Clone, Debug)]
pub struct StepInfo {
    pub name: String,
    pub inputs: Vec<PathBuf>,
    pub outputs: Vec<PathBuf>,
    pub log: PathBuf,
    pub progress_scanning: Option<ProgressScanningInfo>,
}
impl StepInfo {
    pub fn progress_max(&self) -> Option<u32> {
        self.progress_scanning.as_ref().map(|info| info.indicator_max)
    }
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

impl From<&StepInfo> for StepInfo {
    fn from(value: &StepInfo) -> Self {
        value.clone()
    }
}

#[derive(Debug, Deserialize)]
pub struct TargetItem {
    #[allow(unused)]
    path: PathBuf,

    #[serde(rename = "parentStep")]
    pub parent_step: Step,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct WorkflowSpecification {
    #[serde_as(as = "HashMap<_, TargetList>")]
    pub targets: HashMap<String, Vec<TargetItem>>,
}

impl WorkflowSpecification {
    pub fn parse<S: AsRef<str>>(specification: S) -> miette::Result<WorkflowSpecification> {
        let specification = specification.as_ref();

        let input_inspection_file = tempfile::NamedTempFile::new()
            .into_diagnostic()
            .context("failed to create a temporary input inspection file")?;
        let input_inspection_path = input_inspection_file
            .into_temp_path()
            .keep()
            .into_diagnostic()
            .context("failed to keep temporary input inspection file")?;
        let input_inspection_path = PathBuf::from_path_buf(input_inspection_path).expect(&format!(
            "expected the input inspection path to be valid utf8"
        ));
        std::fs::write(&input_inspection_path, specification)
            .into_diagnostic()
            .context(format!(
                "failed to write input to input inspection path `{input_inspection_path}`"
            ))?;

        // start with a pure syntax check, so we can check a pretty json string afterwards
        let specification = serde_json::to_string_pretty(
            &serde_json::from_str::<Value>(specification)
                .with_source_indication(specification)??,
        )
        .expect("expected serialization of serde_json::Value to always succeed");
        std::fs::write(&input_inspection_path, &specification)
            .into_diagnostic()
            .context(format!(
                "failed to write input to input inspection path `{input_inspection_path}`"
            ))?;

        let specification: WorkflowSpecification = serde_json::from_str(specification.as_ref())
            .with_source_indication(specification)??;
        Ok(specification)
    }
}
