use anyhow::{Context, Result};
use std::process::Command;

fn make(target: &str, envs: &[(&str, String)]) -> Result<()> {
    let mut cmd = Command::new("make");
    cmd.arg(target);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let status =
        cmd.status().with_context(|| {
            format!("Failed to run make {target}")
        })?;
    if !status.success() {
        anyhow::bail!("make {target} failed");
    }
    Ok(())
}

pub async fn start(orbits: u8, sats: u8) -> Result<()> {
    let total = orbits as u16 * sats as u16;
    println!(
        "Starting {orbits}x{sats} constellation ({total} satellites)..."
    );

    let envs = [
        ("MAX_ORB", orbits.to_string()),
        ("MAX_SAT", sats.to_string()),
    ];

    make("constellation-build", &envs)?;
    make("constellation-gen", &envs)?;

    let status = Command::new("docker")
        .args([
            "compose",
            "-f",
            "docker-compose.constellation.yml",
            "up",
            "-d",
        ])
        .env("MAX_ORB", orbits.to_string())
        .env("MAX_SAT", sats.to_string())
        .status()
        .context("Failed to start constellation")?;

    if !status.success() {
        anyhow::bail!("docker compose up failed");
    }

    println!("Constellation running.");
    Ok(())
}

pub async fn stop() -> Result<()> {
    println!("Stopping simulation...");

    let status = Command::new("docker")
        .args([
            "compose",
            "-f",
            "docker-compose.constellation.yml",
            "down",
        ])
        .status()
        .context("Failed to stop constellation")?;

    if !status.success() {
        anyhow::bail!("docker compose down failed");
    }

    println!("Simulation stopped.");
    Ok(())
}

pub async fn shell(sat: &str) -> Result<()> {
    let parts: Vec<&str> = sat.split('.').collect();
    if parts.len() != 2 {
        anyhow::bail!(
            "Invalid satellite address. Use 'orbit.sat' format."
        );
    }
    let orb = parts[0];
    let container = format!("orb-{orb}");

    let status = Command::new("docker")
        .args(["exec", "-it", &container, "/bin/bash"])
        .status()
        .context("Failed to open shell")?;

    if !status.success() {
        anyhow::bail!("Shell exited with error");
    }
    Ok(())
}
