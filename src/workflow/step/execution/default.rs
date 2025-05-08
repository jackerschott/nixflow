use serde::Deserialize;
use std::process::Command;

use crate::{commands::clone_command, nix_environment::NixRunCommand};

#[derive(Debug, Deserialize)]
pub struct DefaultExecutor {}

impl DefaultExecutor {
    pub(super) fn execution_command<'s>(&self, target: &Box<dyn NixRunCommand>) -> Command {
        clone_command(
            target
                .command()
                .unwrap_or(Command::new("bash").arg("-c").arg(target.shell_command())),
        )
    }
}

impl std::fmt::Display for DefaultExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "default execution")
    }
}
