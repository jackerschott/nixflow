use default::DefaultExecutor;
use serde::Deserialize;
use slurm::SlurmExecutor;
use std::process::Command;

use crate::nix_environment::NixRunCommand;

mod default;
mod slurm;

#[derive(Debug, Deserialize)]
pub enum Executor {
    Default(DefaultExecutor),
    Slurm(SlurmExecutor),
}

impl Executor {
    pub fn execution_command<'s>(&'s self, target: &Box<dyn NixRunCommand>) -> Command {
        match self {
            Executor::Default(default) => default.execution_command(target),
            Executor::Slurm(slurm) => slurm.execution_command(target),
        }
    }
}

impl Default for Executor {
    fn default() -> Self {
        Executor::Default(DefaultExecutor {})
    }
}

impl std::fmt::Display for Executor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Executor::Default(default) => write!(f, "{default}"),
            Executor::Slurm(slurm) => write!(f, "{slurm}"),
        }
    }
}
