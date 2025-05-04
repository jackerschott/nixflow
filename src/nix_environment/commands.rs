use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use std::process::Command;

use super::FlakeOutput;

pub struct PortableOptions {
    local_cache_parent: PathBuf,
}

impl PortableOptions {
    pub fn new(local_cache_parent: PathBuf) -> Self {
        Self { local_cache_parent }
    }
}

pub fn nix_run_command(
    flake_output: &FlakeOutput,
    portable_options: Option<PortableOptions>,
) -> Command {
    let mut command = Command::new("nix");
    if let Some(portable_options) = portable_options {
        command = Command::new("nix-portable");
        command
            .env("NP_RUNTIME", "bwrap")
            .env("NP_LOCATION", portable_options.local_cache_parent);
        command.arg("nix");
    };

    command
        .arg("run")
        .arg("--show-trace")
        .arg(flake_output.to_string());

    return command;
}

pub fn nix_cache_distribution_command(local_cache: &Path, distributed_cache: &Path) -> Command {
    let mut command = Command::new("tar");
    command
        .arg("--directory")
        .arg(
            local_cache
                .parent()
                .expect("expected cache_local to not be '/' due to user input validation"),
        )
        .arg("--use-compress-program=zstd")
        .arg("--create")
        .arg("--file")
        .arg(distributed_cache)
        .arg(local_cache);

    return command;
}

pub fn nix_distributed_cache_unpacking_command(
    distributed_cache: &Path,
    local_cache_parent: &Path,
) -> Command {
    let mut command = Command::new("tar");
    command
        .arg("--directory")
        .arg(local_cache_parent.parent().unwrap_or(&PathBuf::from("/")))
        .arg("--use-compress-program=zstd")
        .arg("--extract")
        .arg("--file")
        .arg(distributed_cache);

    return command;
}

pub fn nix_version_command(portable_options: Option<PortableOptions>) -> Command {
    let mut command = Command::new("nix");
    if let Some(portable_options) = portable_options {
        command = Command::new("nix-portable");
        command
            .env("NP_RUNTIME", "bwrap")
            .env("NP_LOCATION", portable_options.local_cache_parent);
        command.arg("nix");
    };

    command.arg("--version");

    return command;
}
