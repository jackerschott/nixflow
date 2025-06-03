use camino::Utf8PathBuf as PathBuf;
use commands::{nix_version_command, PortableOptions};
use native::NixNative;
use portable_distributed::NixPortableDistributed;
use std::process::Command;

mod commands;
mod native;
mod portable_distributed;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("nix could neither be executed with `{nix_check_command:?}` nor with `{nix_portable_check_command:?}`")]
    NixUnavailable {
        nix_check_command: Command,
        nix_portable_check_command: Command,
    },
}

pub struct NixRunCommandOptions {
    readonly: bool,
    buffered: bool,
}

impl Default for NixRunCommandOptions {
    fn default() -> Self {
        Self {
            readonly: true,
            buffered: true,
        }
    }
}

impl NixRunCommandOptions {
    pub fn readwrite(mut self) -> Self {
        self.readonly = false;
        self
    }

    pub fn unbuffered(mut self) -> Self {
        self.buffered = false;
        self
    }
}

pub trait NixEnvironment {
    fn run_command(
        &self,
        flake_output: FlakeOutput,
        options: NixRunCommandOptions,
    ) -> Box<dyn NixRunCommand>;
}

pub trait NixRunCommand {
    fn command(&self) -> Option<&Command>;
    fn shell_command(&self) -> String;
}

pub enum FlakeSource {
    _Name(String),
    Path(PathBuf),
}
impl std::fmt::Display for FlakeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlakeSource::_Name(name) => write!(f, "{}", name),
            FlakeSource::Path(path) => write!(f, "./{}", path),
        }
    }
}
pub struct FlakeOutput {
    source: FlakeSource,
    name: Option<String>,
}
impl FlakeOutput {
    pub fn new_default(source: FlakeSource) -> Self {
        Self { source, name: None }
    }

    pub fn new<S: Into<String>>(source: FlakeSource, name: S) -> Self {
        Self {
            source,
            name: Some(name.into()),
        }
    }
}
impl std::fmt::Display for FlakeOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "{}#{}", self.source, name)
        } else {
            write!(f, "{}", self.source)
        }
    }
}

pub fn build_environment(
    cache_local: PathBuf,
    cache_distributed: PathBuf,
    force_nix_portable_usage: bool,
) -> Result<Box<dyn NixEnvironment>, Error> {
    let mut nix_check_command = nix_version_command(None);
    let mut nix_portable_check_command = nix_version_command(Some(PortableOptions::new(
        cache_local.parent().expect("").to_owned(),
    )));

    if !force_nix_portable_usage
        && nix_check_command
            .status()
            .is_ok_and(|status| status.success())
    {
        Ok(Box::new(NixNative {}))
    } else if nix_portable_check_command
        .status()
        .is_ok_and(|status| status.success())
    {
        Ok(Box::new(NixPortableDistributed {
            cache_local,
            cache_distributed,
        }))
    } else {
        Err(Error::NixUnavailable {
            nix_check_command,
            nix_portable_check_command,
        })
    }
}
