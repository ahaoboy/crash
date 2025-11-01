#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use clap::Parser;
use crash::cli::Cli;
use crash::cli::commands::handle;
use crash::log::{LogConfig, LogLevel, init_logger};
use crash::{log_error, log_info};

#[tokio::main]
async fn main() {
    // Initialize logging system
    if let Err(e) = init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        // Continue without logging rather than failing
    }

    log_info!("Crash application starting");

    // Run the application and handle errors
    if let Err(e) = run().await {
        log_error!("Application error: {}", e);
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    log_info!("Crash application exiting");
}

/// Initialize the logging system
fn init_logging() -> Result<()> {
    let log_dir = crash::config::get_log_dir();

    let config = LogConfig {
        log_dir,
        log_level: LogLevel::Info,
        max_file_size: 10 * 1024 * 1024, // 10MB
        max_files: 5,
    };

    init_logger(config)?;
    Ok(())
}

/// Main application logic
async fn run() -> Result<()> {
    let cli = Cli::parse();

    log_info!("Parsed CLI arguments");
    handle(cli.command).await?;
    Ok(())
}
