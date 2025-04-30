use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

mod workflow;

fn generate_workflow_steps() -> Result<String> {
    let output = Command::new("nix")
        .arg("run")
        .arg("--show-trace")
        .arg("./examples")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .context("failed to generate workflow steps using `nix run --show-trace workflow`")?;

    if !output.status.success() {
        bail!("failed to generate workflow steps using `nix run --show-trace workflow`");
    }

    let workflow_steps = String::from_utf8(output.stdout).expect("expected nix run output to always be valid utf8");

    Ok(workflow_steps)
}

fn main() -> Result<()> {
    generate_workflow_steps()?;

    Ok(())
}
