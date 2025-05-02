use anyhow::{Context, Result};
use camino::Utf8PathBuf as PathBuf;
use clap::Parser;
use nix_environment::NixEnvironment;
use serde::Deserialize;
use workflow::{generate_workflow_specification, WorkflowSpecification};

mod nix_environment;
mod workflow;

#[derive(Deserialize)]
struct GlobalConfig {
    apptainer_args: Vec<String>,
    nix_container_url: String,
    nix_binary_cache_path: PathBuf,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(value_name = "workflow")]
    workflow_path: PathBuf,

    #[arg(long)]
    force_container_usage: bool,
}

fn main() -> Result<()> {
    // generate workflow steps
    let config: GlobalConfig = serde_yaml::from_str(
        &std::fs::read_to_string("workflow/config.yaml").context("failed to read configuration")?,
    )
    .context("failed to parse configuration")?;
    let cli = Cli::parse();
    let flake_path = PathBuf::from("workflow");

    let nix_environment = if cli.force_container_usage {
        NixEnvironment::new_container(
            config.nix_container_url,
            config.nix_binary_cache_path,
            config.apptainer_args,
        )
        .context("failed to setup container nix environment")?
    } else {
        NixEnvironment::new(
            config.nix_container_url,
            config.nix_binary_cache_path,
            config.apptainer_args,
        )
        .context("failed to setup nix environment")?
    };

    let workflow_specification = generate_workflow_specification(&nix_environment, &flake_path)
        .context("failed to generate workflow specification")?;
    let workflow_specification = WorkflowSpecification::parse(&workflow_specification)
        .context("failed to parse workflow specification")?;

    println!("{:?}", workflow_specification);

    Ok(())
}
