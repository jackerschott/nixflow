use serde::Deserialize;
use std::process::Command;

use super::ExecutionCommand;
use crate::{commands::clone_command, nix_environment::NixRunCommand, workflow::step::Step};

#[derive(Debug, Deserialize)]
pub struct DefaultExecutor {}

impl DefaultExecutor {
    pub(super) fn execution_command<'s>(&self, step: &'s Step, target: &Box<dyn NixRunCommand>) -> ExecutionCommand<'s> {
        ExecutionCommand {
            command: clone_command(
                target
                    .command()
                    .unwrap_or(Command::new("bash").arg("-c").arg(target.shell_command())),
            ),
            step,
        }
    }
}

impl std::fmt::Display for DefaultExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "default execution")
    }
}
