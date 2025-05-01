use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use clap::Parser;
use nix_environment::NixEnvironment;
use serde::Deserialize;

mod nix_environment;
mod workflow;

//fn generate_workflow_steps() -> Result<String> {
//    let output = Command::new("nix")
//        .arg("run")
//        .arg("--show-trace")
//        .arg("./examples")
//        .stdout(Stdio::piped())
//        .stderr(Stdio::inherit())
//        .output()
//        .context("failed to generate workflow steps using `nix run --show-trace workflow`")?;
//
//    if !output.status.success() {
//        bail!("failed to generate workflow steps using `nix run --show-trace workflow`");
//    }
//
//    let workflow_steps =
//        String::from_utf8(output.stdout).expect("expected nix run output to always be valid utf8");
//
//    Ok(workflow_steps)
//}

fn generate_workflow_steps(nix_environment: &NixEnvironment, flake_path: &Path) -> Result<String> {
    let mut command = if nix_environment.is_containerized() {
        Command::new("apptainer")
    } else {
        Command::new("nix")
    };

    let home = std::env::var("HOME").context("failed to read HOME")?;
    let output = match nix_environment {
        NixEnvironment::Container {
            apptainer_args, ..
        } => command
            .arg("exec")
            .arg("--contain")
            .arg("--env")
            .arg("NIX_CONFIG=experimental-features = nix-command flakes")
            .arg("--overlay")
            .arg(nix_environment.store_image_path())
            .arg("--bind")
            .arg(format!("{flake_path}:{home}/workflow"))
            .args(apptainer_args)
            .arg(nix_environment.nix_container_cache_path())
            .arg("nix"),
        NixEnvironment::Native => &mut command,
    }
    .arg("run")
    .arg("--show-trace")
    .arg("./workflow")
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())
    .output()
    .context("failed to generate workflow steps")?;

    if !output.status.success() {
        bail!("failed to generate workflow steps using `nix run --show-trace workflow`");
    }

    let workflow_steps =
        String::from_utf8(output.stdout).expect("expected nix run output to always be valid utf8");

    Ok(workflow_steps)
}

#[derive(Deserialize)]
struct GlobalConfig {
    apptainer_args: Vec<String>,
    nix_container_url: String,
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
        NixEnvironment::new_container(config.nix_container_url, config.apptainer_args)
            .context("failed to setup container nix environment")?
    } else {
        NixEnvironment::new(config.nix_container_url, config.apptainer_args)
            .context("failed to setup nix environment")?
    };

    println!(
        "{}",
        generate_workflow_steps(&nix_environment, &flake_path)?
    );

    Ok(())
}
