use anyhow::{Context, Result};
use std::process::Command;

pub async fn run(
    scenario: &str,
    output: &str,
    fmt: &str,
) -> Result<()> {
    println!("Generating sensor data from {scenario}...");

    let status = Command::new("uv")
        .args([
            "run",
            "eosim",
            "wildfire",
            scenario,
            "-o",
            output,
            "--fmt",
            fmt,
        ])
        .current_dir("tools/eosim")
        .status()
        .context("Failed to run eosim (is uv installed?)")?;

    if !status.success() {
        anyhow::bail!("eosim failed");
    }

    println!("Data written to {output}/");
    Ok(())
}
