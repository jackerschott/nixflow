use std::collections::HashMap;

use camino::Utf8PathBuf as PathBuf;
use miette::{Context, IntoDiagnostic};
use parsing::{TargetList, WithSourceIndication};
use serde::Deserialize;
use serde_json::Value;
use serde_with::serde_as;

use super::step::Step;

mod parsing;

#[derive(Debug, Deserialize)]
pub struct TargetItem {
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
