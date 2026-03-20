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

    let status = Command::new("docker")
        .args([
            "run", "-d",
            "--name", "leodos-constellation",
            "-e", &format!("MAX_ORB={orbits}"),
            "-e", &format!("MAX_SAT={sats}"),
            "-p", "1234:1234/udp",
            "-p", "1235:1235/udp",
            "--sysctl", "fs.mqueue.msg_max=1000",
            "leodos-sat:latest",
        ])
        .status()
        .context("Failed to start constellation")?;

    if !status.success() {
        anyhow::bail!("docker run failed");
    }

    println!("Constellation running ({total} satellites in one container).");
    Ok(())
}

pub async fn stop() -> Result<()> {
    println!("Stopping simulation...");

    let status = Command::new("docker")
        .args(["stop", "leodos-constellation"])
        .status()
        .context("Failed to stop constellation")?;

    if !status.success() {
        anyhow::bail!("docker stop failed");
    }

    let _ = Command::new("docker")
        .args(["rm", "leodos-constellation"])
        .status();

    println!("Simulation stopped.");
    Ok(())
}

pub async fn shell(_sat: &str) -> Result<()> {
    let status = Command::new("docker")
        .args(["exec", "-it", "leodos-constellation", "/bin/bash"])
        .status()
        .context("Failed to open shell")?;

    if !status.success() {
        anyhow::bail!("Shell exited with error");
    }
    Ok(())
}
