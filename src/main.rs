#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

// FIXME: run task without window on windows
// #![cfg_attr(
//     all(target_os = "windows", not(debug_assertions)),
//     windows_subsystem = "windows"
// )]

use anyhow::Result;
use clap::Parser;
use crash::cli::Cli;
use crash::cli::commands::handle;
use crash::log::{LogConfig, init_logger};
use crash::{log_error, log_info};

#[cfg(windows)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{
        AttachConsole, GetStdHandle,
        SetStdHandle, ATTACH_PARENT_PROCESS,
        STD_OUTPUT_HANDLE, STD_ERROR_HANDLE,
    };
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;

    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return;
        }
        let stdout = GetStdHandle(STD_OUTPUT_HANDLE);
        let stderr = GetStdHandle(STD_ERROR_HANDLE);

        if stdout != INVALID_HANDLE_VALUE {
            SetStdHandle(STD_OUTPUT_HANDLE, stdout);
        }
        if stderr != INVALID_HANDLE_VALUE {
            SetStdHandle(STD_ERROR_HANDLE, stderr);
        }
    }
}

#[tokio::main]
async fn main() {
    #[cfg(windows)]
    attach_console();

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
    let config = LogConfig::default();

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
