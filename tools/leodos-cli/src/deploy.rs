use anyhow::Result;
use std::path::Path;

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

    let remote_path = format!("/cf/{app}.so");

    println!("Deploying {app} from {so_path}...");

    // Step 1: Upload .so via fs_srv file protocol
    crate::fs::put(host, &so_path, &remote_path).await?;

    // Step 2: Send ES RELOAD_APP command
    // CFE_ES_CMD_MID = 0x1806, RELOAD_APP_CC = 7
    // Payload: app name (20 bytes) + filename (64 bytes)
    let es_cmd_mid: u16 = 0x1806;
    let reload_cc: u8 = 7;

    let mut payload = [0u8; 84];
    let name_bytes = app.as_bytes();
    let file_bytes = remote_path.as_bytes();

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
