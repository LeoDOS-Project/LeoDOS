use anyhow::{Context, Result};
use std::process::Command;

fn make(target: &str) -> Result<()> {
    let status = Command::new("make")
        .arg(target)
        .status()
        .with_context(|| format!("Failed to run make {target}"))?;

    if !status.success() {
        anyhow::bail!("make {target} failed");
    }
    Ok(())
}

pub async fn run() -> Result<()> {
    println!("Building Docker image...");
    make("nos3-build")?;

    println!("\nConfiguring NOS3...");
    make("nos3-config")?;

    println!("\nBuilding simulators...");
    make("nos3-build-sim")?;

    println!("\nBuilding flight software...");
    make("nos3-build-fsw")?;

    println!("\nBuild complete.");
    Ok(())
}
