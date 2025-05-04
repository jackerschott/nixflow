use serde::Deserialize;
use std::{
    process::Command,
    time::Duration,
};

use super::ExecutionCommand;
use crate::{nix_environment::NixRunCommand, workflow::step::Step};

#[derive(Debug, Deserialize)]
pub struct SlurmExecutor {
    account: String,

    #[serde(flatten)]
    options: SlurmRunOptions,
}

impl SlurmExecutor {
    pub(super) fn execution_command<'s>(&self, step: &'s Step, target: &Box<dyn NixRunCommand>) -> ExecutionCommand<'s> {
        ExecutionCommand {
            command: slurm_run_command(
                target
                    .command()
                    .unwrap_or(Command::new("bash").arg("-c").arg(target.shell_command())),
                &self.account,
                &self.options,
            ),
            step,
        }
    }
}

impl std::fmt::Display for SlurmExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "slurm execution")
    }
}

#[derive(Debug, Deserialize)]
pub struct SlurmRunOptions {
    #[serde(default)]
    quality_of_service: Option<String>,

    #[serde(default)]
    constraint: Option<String>,

    runtime: Duration,

    #[serde(default)]
    partitions: Option<Vec<String>>,

    cpu_count: u16,
    gpu_count: u16,
}

pub fn slurm_run_command(target: &Command, account: &str, options: &SlurmRunOptions) -> Command {
    let mut command = Command::new("srun");
    for (name, value) in target.get_envs() {
        match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }

    command.arg("--account").arg(account);

    if let Some(service_quality) = &options.quality_of_service {
        command.arg("--qos").arg(service_quality);
    }

    if let Some(constraint) = &options.constraint {
        command.arg("--constraint").arg(constraint);
    }

    command
        .arg("--time")
        .arg(options.runtime.format_slurm_time());

    if let Some(partitions) = &options.partitions {
        command.arg("--partition").arg(partitions.join(","));
    }

    command
        .arg("--cpus-per-task")
        .arg(options.cpu_count.to_string())
        .arg("--gpus")
        .arg(options.gpu_count.to_string());

    command.arg(target.get_program()).args(target.get_args());

    return command;
}

trait FormatSlurmTime {
    fn format_slurm_time(&self) -> String;
}

impl FormatSlurmTime for Duration {
    fn format_slurm_time(&self) -> String {
        let total_seconds = self.as_secs();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
}
