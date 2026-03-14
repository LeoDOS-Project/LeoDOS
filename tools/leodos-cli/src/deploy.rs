use anyhow::Result;
use std::path::Path;
use std::process::Command;

pub async fn run(
    app: &str,
    file: Option<&str>,
    host: &str,
    port: u16,
) -> Result<()> {
    let so_path = match file {
        Some(f) => f.to_string(),
        None => {
            let candidate = format!(
                "apps/{app}/fsw/target/release/lib{app}.so"
            );
            if !Path::new(&candidate).exists() {
                anyhow::bail!(
                    "No .so found at {candidate}. \
                     Build with `leodos build --app {app} --release` first, \
                     or specify --file."
                );
            }
            candidate
        }
    };

    if !Path::new(&so_path).exists() {
        anyhow::bail!("File not found: {so_path}");
    }

    println!("Deploying {app} from {so_path}...");

    // Step 1: Copy .so to the running container's /cf/ directory
    // This assumes the FSW container is named "fsw" in docker-compose.
    let container = "fsw";
    let dest = format!("{container}:/cf/{app}.so");

    let status = Command::new("docker")
        .args(["cp", &so_path, &dest])
        .status()?;

    if !status.success() {
        anyhow::bail!(
            "Failed to copy {so_path} to container {container}"
        );
    }

    println!("Copied to {dest}");

    // Step 2: Send ES RELOAD_APP command
    // CFE_ES_CMD_MID = 0x1806, RELOAD_APP_CC = 7
    // Payload: app name (20 bytes) + filename (64 bytes)
    let es_cmd_mid: u16 = 0x1806;
    let reload_cc: u8 = 7;

    let mut payload = [0u8; 84];
    let name_bytes = app.as_bytes();
    let file_bytes = format!("/cf/{app}.so");
    let file_bytes = file_bytes.as_bytes();

    let name_len = name_bytes.len().min(19);
    payload[..name_len].copy_from_slice(&name_bytes[..name_len]);

    let file_len = file_bytes.len().min(63);
    payload[20..20 + file_len]
        .copy_from_slice(&file_bytes[..file_len]);

    println!(
        "Sending ES RELOAD_APP command for '{app}'..."
    );
    crate::tc::send(
        host,
        port,
        es_cmd_mid,
        reload_cc,
        &payload,
        false,
    )
    .await?;

    println!("Deploy complete. App should restart shortly.");
    Ok(())
}
