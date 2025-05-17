use anyhow::{Context, Result};
use camino::Utf8PathBuf as PathBuf;
use clap::Parser;
use nix_environment::build_environment;
use serde::Deserialize;
use workflow::{
    graph::{GraphExecutor, JobGraph},
    WorkflowSpecification,
};

mod commands;
mod nix_environment;
mod utils;
mod workflow;

#[derive(Deserialize)]
struct GlobalConfig {
    nix_local_cache_directory_path: PathBuf,
    nix_distributed_cache_path: PathBuf,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(name = "workflow")]
    workflow_flake_path: PathBuf,

    #[arg(long)]
    force_nix_portable_usage: bool,
}

fn main() -> Result<()> {
    // generate workflow steps
    let cli = Cli::parse();
    let config: GlobalConfig = serde_yaml::from_str(
        &std::fs::read_to_string(format!(
            "{workflow}/config.yaml",
            workflow = cli.workflow_flake_path
        ))
        .context("failed to read configuration")?,
    )
    .context("failed to parse configuration")?;

    let nix_environment = build_environment(
        config.nix_local_cache_directory_path,
        config.nix_distributed_cache_path,
        cli.force_nix_portable_usage,
    )
    .context("failed to build nix environment")?;

    let workflow_specification =
        WorkflowSpecification::generate(&nix_environment, &cli.workflow_flake_path)
            .context("failed to generate workflow specification")?;

    let job_graph = JobGraph::new(
        workflow_specification,
        &nix_environment,
        &cli.workflow_flake_path,
    );

    let _ = GraphExecutor::new(job_graph.job_count(), 3, false).execute(job_graph);

    Ok(())
}
