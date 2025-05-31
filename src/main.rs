use camino::Utf8PathBuf as PathBuf;
use clap::Parser;
use miette::{Context, IntoDiagnostic, Result};
use nix_environment::build_environment;
use serde::Deserialize;
use workflow::{
    generate_specification_string,
    graph::{GraphExecutor, JobGraph},
    specification::WorkflowSpecification,
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
        .into_diagnostic()
        .context("failed to read configuration")?,
    )
    .into_diagnostic()
    .context("failed to parse configuration")?;

    let nix_environment = build_environment(
        config.nix_local_cache_directory_path,
        config.nix_distributed_cache_path,
        cli.force_nix_portable_usage,
    )
    .into_diagnostic()
    .context("failed to build nix environment")?;

    let specification_string =
        &generate_specification_string(&nix_environment, &cli.workflow_flake_path)
            .into_diagnostic()
            .context(format!(
                "failed to generate workflow specification from `{workflow_flake}`",
                workflow_flake = cli.workflow_flake_path
            ))?;

    let workflow_specification = WorkflowSpecification::parse(specification_string)
        .context("failed to generate workflow specification")?;

    let job_graph = JobGraph::new(
        workflow_specification,
        &nix_environment,
        &cli.workflow_flake_path,
    );

    let _ = GraphExecutor::new(job_graph.job_count(), 3, false).execute(job_graph);

    Ok(())
}
