use anyhow::{Context, Result};
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use serde::Deserialize;
use serde_with::{serde_as, OneOrMany};
use std::{
    collections::HashMap,
    process::{Command, Stdio},
};
use step::Step;

use crate::nix_environment::{FlakeOutput, FlakeSource, NixEnvironment, NixRunCommandOptions};

pub mod graph;
pub mod step;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("failed to execute `{command}`\n{io_error}")]
    Io {
        command: String,
        io_error: std::io::Error,
    },

    #[error("failed to execute `{command}`, exit code {code} is non-zero")]
    NonZeroExitCode { command: String, code: i32 },

    #[error("failed to execute `{command}`, terminated by a signal")]
    SignalTermination { command: String },
}
impl CommandError {
    pub fn new_io(command: &Command, io_error: std::io::Error) -> Self {
        Self::Io {
            command: format!("{command:?}"),
            io_error,
        }
    }

    pub fn new_non_zero_exit_code(command: &Command, code: i32) -> Self {
        Self::NonZeroExitCode {
            command: format!("{command:?}"),
            code,
        }
    }

    pub fn new_signal_termination(command: &Command) -> Self {
        Self::SignalTermination {
            command: format!("{command:?}"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error(
        "failed to generate workflow specification, see above for the associated nix error\n{0}"
    )]
    SpecificationGeneration(CommandError),
}

#[derive(Debug, Deserialize)]
struct Target {
    #[allow(unused)]
    path: PathBuf,

    #[serde(rename = "parentStep")]
    parent_step: Step,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(transparent)]
struct TargetList {
    #[serde_as(as = "OneOrMany<_>")]
    targets: Vec<Target>,
}

#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct WorkflowSpecification {
    targets: HashMap<String, TargetList>,
}

impl WorkflowSpecification {
    pub fn parse(specification: &str) -> Result<Self> {
        // TODO: validate workflow specification, e.g. are input output paths file paths?
        // are they not the root?
        //println!("{}", specification);
        //let v: Value = serde_json::from_str(specification)?;
        //let specification = serde_json::to_string_pretty(&v)?;
        //println!("{}", specification);
        //println!("");
        Ok(serde_json::from_str(&specification).context("failed to deserialize specification")?)
    }

    fn generate_specification_string(
        nix_environment: &Box<dyn NixEnvironment>,
        flake_path: &Path,
    ) -> Result<String, WorkflowError> {
        let mut command = Command::new("bash");
        let nix_run_command = nix_environment.run_command(
            FlakeOutput::new_default(FlakeSource::Path(flake_path.to_owned())),
            NixRunCommandOptions::default().readwrite(),
        );

        command.arg("-c").arg(nix_run_command.shell_command());
        let output = command.stderr(Stdio::inherit()).output().map_err(|err| {
            WorkflowError::SpecificationGeneration(CommandError::new_io(&command, err))
        })?;

        match output.status.code() {
            Some(0) => {}
            Some(code) => {
                assert!(!output.status.success());
                return Err(WorkflowError::SpecificationGeneration(
                    CommandError::new_non_zero_exit_code(&command, code),
                ));
            }
            None => {
                return Err(WorkflowError::SpecificationGeneration(
                    CommandError::new_signal_termination(&command),
                ));
            }
        }

        let workflow_steps = String::from_utf8(output.stdout)
            .expect("expected nix run output to always be valid utf8");

        Ok(workflow_steps)
    }

    pub fn generate(
        nix_environment: &Box<dyn NixEnvironment>,
        workflow_flake: &Path,
    ) -> Result<Self> {
        let specification_string =
            &Self::generate_specification_string(nix_environment, workflow_flake).context(
                format!("failed to generate workflow specification from `{workflow_flake}`"),
            )?;

        Self::parse(specification_string)
            .context(format!("failed to parse generated specification string"))
    }
}
