use anyhow::{bail, Context, Result};
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use serde::Deserialize;
use serde_json::Value;
use serde_with::{serde_as, KeyValueMap, OneOrMany};
use std::{
    collections::HashMap,
    process::{Command, Stdio},
};

use crate::nix_environment::NixEnvironment;

#[derive(Debug, Deserialize)]
enum OutputPaths {
    Single(),
    Multiple(),
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(transparent)]
struct InputList {
    inputs: Vec<Input>,
}

trait InputOutput {
    fn path() -> &Path;
}

impl InputList {
    fn paths(&self) -> Result<Vec<PathBuf>> {
        self.inputs.iter().map(|input| {
            Ok(camino::absolute_utf8(input.path.clone())
                .context(format!(
                    "failed to convert {path} to an absolute path",
                    path = input.path
                ))?
                .parent()
                .map(|x| x.to_owned())
                .expect(&format!(
                    "expected {path} to have a parent, since its absolute \
                        and checked to not be '/' in the workflow specification
                        validation",
                    path = input.path.clone()
                )))
        }).collect()
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(transparent)]
struct OutputList {
    #[serde_as(as = "OneOrMany<_>")]
    outputs: Vec<Output>,
}

#[derive(Debug, Deserialize)]
struct Input {
    path: PathBuf,

    #[serde(rename = "parentStep")]
    parent_step: Step,
}
#[derive(Debug, Deserialize)]
#[serde(transparent)]
struct Output {
    path: PathBuf,
}

#[serde_as]
#[derive(Debug, Deserialize)]
struct Step {
    name: String,

    #[serde(default)]
    inputs: HashMap<String, InputList>,

    outputs: HashMap<String, OutputList>,

    #[serde(rename = "run")]
    run_binary_path: PathBuf,
}

#[serde_as]
#[derive(Debug, Deserialize)]
struct Target {
    #[serde(rename = "$key$")]
    name: String,

    path: PathBuf,

    #[serde(rename = "parentStep")]
    parent_step: Step,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct WorkflowSpecification {
    #[serde_as(as = "KeyValueMap<_>")]
    #[serde(flatten)]
    targets: Vec<Target>,
}

impl WorkflowSpecification {
    pub fn parse(specification: &str) -> Result<Self> {
        // TODO: validate workflow specification, e.g. are input output paths file paths?
        // are they not the root?
        let v: Value = serde_json::from_str(specification)?;
        let specification = serde_json::to_string_pretty(&v)?;
        println!("{}", specification);
        println!("");
        Ok(serde_json::from_str(&specification).context("failed to deserialize specification")?)
    }

    fn generate_specification_string(
        nix_environment: &NixEnvironment,
        flake_path: &Path,
    ) -> Result<String> {
        let mut command = if nix_environment.is_containerized() {
            Command::new("apptainer")
        } else {
            Command::new("nix")
        };
    
        let home = std::env::var("HOME").context("failed to read HOME")?;
        let output = match nix_environment {
            NixEnvironment::Container {
                nix_binary_cache_path,
                apptainer_args,
                ..
            } => command
                .arg("exec")
                .arg("--cleanenv")
                .arg("--contain")
                .arg("--env")
                .arg("NIX_CONFIG=experimental-features = nix-command flakes")
                .arg("--env")
                .arg(format!("XDG_CACHE_HOME={nix_binary_cache_path}"))
                .arg("--overlay")
                .arg(nix_environment.store_image_path())
                .arg("--bind")
                .arg(format!("{flake_path}:{home}/workflow"))
                .args(apptainer_args)
                .arg(nix_environment.nix_container_cache_path())
                .arg("nix"),
            NixEnvironment::Native => &mut command,
        }
        .arg("run")
        .arg("--show-trace")
        .arg("./workflow")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .context("failed to generate workflow steps")?;
    
        if !output.status.success() {
            bail!("failed to generate workflow steps using `nix run --show-trace workflow`");
        }
    
        let workflow_steps =
            String::from_utf8(output.stdout).expect("expected nix run output to always be valid utf8");
    
        Ok(workflow_steps)
    }

    pub fn generate(
        nix_environment: &NixEnvironment,
        flake_path: &Path,
    ) -> Result<Self> {
        let specification_string = &Self::generate_specification_string(nix_environment, flake_path)
            .context(format!("failed to generate workflow specification from `{flake_path}`"))?;

        Self::parse(specification_string)
            .context(format!("failed to parse generated specification string"))
    }
}

pub fn execute_workflow_step(step: Step, nix_environment: &NixEnvironment) -> Result<()> {
    let input_output_directory_paths = Iterator::chain(
        step.inputs
            .values()
            .flat_map(|input_list| input_list.inputs.iter())
            .map(|input| {
                Ok(camino::absolute_utf8(input.path.clone())
                    .context(format!(
                        "failed to convert {path} to an absolute path",
                        path = input.path
                    ))?
                    .parent()
                    .map(|x| x.to_owned())
                    .expect(&format!(
                        "expected {path} to have a parent, since its absolute \
                        and checked to not be '/' in the workflow specification
                        validation",
                        path = input.path.clone()
                    )))
            }),
        step.outputs
            .values()
            .flat_map(|output_list| output_list.outputs.iter())
            .map(|output| {
                Ok(camino::absolute_utf8(output.path.clone())
                    .context(format!(
                        "failed to convert {path} to an absolute path",
                        path = output.path
                    ))?
                    .parent()
                    .map(|x| x.to_owned())
                    .expect(&format!(
                        "expected {path} to have a parent, since its absolute \
                        and checked to not be '/' in the workflow specification
                        validation",
                        path = output.path
                    )))
            }),
    )
    .collect::<Result<_>>()
    .context("bla")?;

    let mut command = nix_environment
        .nix_store_binary_execution_command(&step.run_binary_path, &input_output_directory_paths);

    if !command
        .status()
        .context(format!(
            "failed to execute {name} run binary",
            name = step.name
        ))?
        .success()
    {
        bail!(
            "workflow step {name} finished with a non-zero exit status",
            name = step.name
        );
    }

    Ok(())
}

pub fn execute_workflow(specification: WorkflowSpecification) -> Result<()> {
    Ok(())
}
