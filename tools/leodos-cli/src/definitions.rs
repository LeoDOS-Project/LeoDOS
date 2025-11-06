use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

/// Represents the entire `commands.toml` definition file.
#[derive(Deserialize, Debug)]
pub struct Definitions {
    pub network: NetworkConfig,
    pub mids: HashMap<String, u16>,
    #[serde(rename = "command")]
    pub commands: Vec<CommandDef>,
    #[serde(rename = "telemetry")]
    pub telemetry: Vec<TelemetryDef>,
}

/// Represents the `[network]` table in the TOML file.
#[derive(Deserialize, Debug)]
pub struct NetworkConfig {
    pub default_host: String,
    pub command_port: u16,
    pub telemetry_listen_port: u16,
}

/// Represents a single `[[command]]` definition.
#[derive(Deserialize, Debug)]
pub struct CommandDef {
    pub name: String,
    pub mid: String, // The string key that maps to the `mids` table
    pub function_code: u16,
    pub description: String,
    // We keep `params` optional for now. A more advanced tool would parse this.
    // pub params: Option<Vec<ParamDef>>,
}

/// Represents a single `[[telemetry]]` definition.
#[derive(Deserialize, Debug)]
pub struct TelemetryDef {
    pub name: String,
    pub mid: String, // The string key that maps to the `mids` table
    pub description: String,
    pub payload: Vec<PayloadFieldDef>,
}

/// Represents a single field within a telemetry payload.
#[derive(Deserialize, Debug)]
pub struct PayloadFieldDef {
    pub name: String,
    pub data_type: String, // e.g., "u8", "u16", "f32"
    pub offset: usize,
}

/// Loads and parses the TOML definition file.
pub fn load_definitions(path: &str) -> Result<Definitions> {
    let toml_str = fs::read_to_string(path)
        .with_context(|| format!("Failed to read definitions file at '{}'", path))?;
    let defs: Definitions = toml::from_str(&toml_str)
        .with_context(|| "Failed to parse TOML from definitions file")?;
    Ok(defs)
}
