use anyhow::{Context, Result};
use std::process::Command;
use std::time::Duration;

fn make(target: &str) -> Result<()> {
    eprintln!("→ make {target}");
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
    // Check Docker is available before starting a long build.
    // Spawn in a thread with a timeout to avoid hanging if the
    // daemon socket exists but the daemon is unresponsive.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let ok = Command::new("docker")
            .args(["version", "--format", "{{.Server.Version}}"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        let _ = tx.send(ok);
    });
    let docker_ok = rx.recv_timeout(Duration::from_secs(5)).unwrap_or(false);
    if !docker_ok {
        anyhow::bail!("Docker is not running. Start Docker and try again.");
    }

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
