use anyhow::Result;
use clap::Parser;
use clap::Subcommand;

mod tc;
mod tm;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Send a CFE Telecommand packet.
    Send {
        /// IP address of the cFS host
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// UDP port for sending commands
        #[arg(long, default_value_t = 1234)]
        port: u16,

        /// CFE Message ID for the command (e.g., "0x18F8")
        #[arg(long)]
        message_id: String,

        /// CFE command function code
        #[arg(long)]
        function_code: u8,

        /// (Future Use) List of parameters for the payload
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        params: Vec<String>,
    },

    /// Listen for and decode CFE Telemetry packets.
    Listen {
        /// UDP port to listen on for telemetry
        #[arg(long, default_value_t = 1235)]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Send {
            host,
            port,
            message_id,
            function_code,
            params,
        } => {
            // Parse the Message ID from a string, allowing for "0x" prefix
            let mid_val = u16::from_str_radix(message_id.trim_start_matches("0x"), 16)?;

            println!("Sending command to {}:{}", host, port);
            println!("  MID: {:#06x}, Function Code: {}", mid_val, function_code);
            tc::send(host, *port, mid_val, *function_code, params).await?;
        }
        Commands::Listen { port } => {
            println!("Listening for telemetry on port {}...", port);
            tm::listen(*port).await?;
        }
    }

    Ok(())
}
