use std::process::Command;

use crate::commands::shell_command;

use super::{
    commands::nix_run_command, FlakeOutput, NixEnvironment, NixRunCommand, NixRunCommandOptions,
};

pub struct NixNative {}

impl NixEnvironment for NixNative {
    fn run_command(
        &self,
        flake_output: FlakeOutput,
        options: NixRunCommandOptions,
    ) -> Box<dyn NixRunCommand> {
        Box::new(NixNativeRunCommand {
            run: nix_run_command(&flake_output, None, options.buffered),
        })
    }
}

pub struct NixNativeRunCommand {
    run: Command,
}

impl NixRunCommand for NixNativeRunCommand {
    fn command(&self) -> Option<&Command> {
        Some(&self.run)
    }

    fn shell_command(&self) -> String {
        shell_command(&self.run)
    }
}
