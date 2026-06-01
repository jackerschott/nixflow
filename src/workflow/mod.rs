use camino::Utf8Path as Path;
use std::process::{Command, Stdio};

use crate::nix_environment::{FlakeOutput, FlakeSource, NixEnvironment, NixRunCommandOptions};

pub mod graph;
pub mod job;
pub mod specification;


#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error(
        "failed to generate workflow specification, see above for the associated nix error\n{0}"
    )]
    SpecificationGeneration(CommandError),
}

pub fn generate_specification_string(
    nix_environment: &Box<dyn NixEnvironment>,
    flake_path: &Path,
    profile: &str,
) -> Result<String, WorkflowError> {
    let mut command = Command::new("bash");
    let nix_run_command = nix_environment.run_command(
        FlakeOutput::new(
            FlakeSource::Path(flake_path.to_owned()),
            format!("specificationPrinter.{profile}"),
        ),
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

    let workflow_steps =
        String::from_utf8(output.stdout).expect("expected nix run output to always be valid utf8");

    Ok(workflow_steps)
}
