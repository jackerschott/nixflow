use camino::Utf8PathBuf as PathBuf;
use std::process::Command;

use crate::commands::shell_command;

use super::{
    commands::{
        nix_cache_distribution_command, nix_distributed_cache_unpacking_command, nix_run_command,
        PortableOptions,
    },
    NixEnvironment, FlakeOutput, NixRunCommand,
};

pub struct NixPortableDistributed {
    pub(super) cache_local: PathBuf,
    pub(super) cache_distributed: PathBuf,
}

impl NixEnvironment for NixPortableDistributed {
    fn run_command(&self, flake_output: FlakeOutput, readonly: bool) -> Box<dyn NixRunCommand> {
        let cache_local_parent = self
            .cache_local
            .parent()
            .expect("expected cache_local to not be '/' due to user input validation");

        Box::new(NixPortableDistributedRunCommand {
            run: nix_run_command(
                &flake_output,
                Some(PortableOptions::new(cache_local_parent.to_owned())),
            ),
            unpack_cache: nix_distributed_cache_unpacking_command(
                &self.cache_distributed,
                cache_local_parent,
            ),
            distribute_cache: (!readonly).then_some(nix_cache_distribution_command(
                &self.cache_local,
                &self.cache_distributed,
            )),
        })
    }
}

pub struct NixPortableDistributedRunCommand {
    run: Command,
    unpack_cache: Command,
    distribute_cache: Option<Command>,
}

impl NixRunCommand for NixPortableDistributedRunCommand {
    fn command(&self) -> Option<&Command> {
        return None;
    }

    fn shell_command(&self) -> String {
        if let Some(distribute_cache) = &self.distribute_cache {
            format!(
                "{unpack_cache} && {run} && {distribute_cache}",
                unpack_cache = shell_command(&self.unpack_cache),
                run = shell_command(&self.run),
                distribute_cache = shell_command(&distribute_cache)
            )
        } else {
            format!(
                "{unpack_cache} && {run}",
                unpack_cache = shell_command(&self.unpack_cache),
                run = shell_command(&self.run)
            )
        }
    }
}
