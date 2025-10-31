// CLI module for command-line interface

use clap::{Parser, Subcommand};
use github_proxy::Proxy;

pub mod commands;
pub mod output;

pub use commands::CommandHandler;
pub use output::OutputFormatter;

/// Main CLI structure
#[derive(Parser)]
#[command(name = "crash", version)]
#[command(about = "A tool for managing proxy cores like Clash/Mihomo/SingBox", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available CLI commands
#[derive(Subcommand)]
pub enum Commands {
    /// Install proxy core and UI components
    Install {
        /// Force reinstallation even if already installed
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },

    /// Set the GitHub proxy to use for downloads
    Proxy {
        /// Proxy type (e.g., Direct, Ghproxy, etc.)
        proxy: Proxy,
    },

    /// Start the proxy service
    Start,

    /// Stop the proxy service
    Stop,

    /// Restart the proxy service
    Restart,

    /// Show service status
    Status,

    /// Manage scheduled tasks
    Task,

    /// Run scheduled update task
    RunTask,

    /// Set the configuration URL
    Url {
        /// Configuration file URL
        url: String,
    },

    /// Update configuration from URL
    UpdateUrl {
        /// Force update even if file exists
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },

    /// Update GeoIP databases
    UpdateGeo {
        /// Force update even if files exist
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },

    /// Update configuration from stored URL
    Update,

    /// Set the web UI type
    Ui {
        /// UI type (Metacubexd, Zashboard, Yacd)
        ui: String,
    },

    /// Set the web controller host
    Host {
        /// Host address (e.g., :9090)
        host: String,
    },

    /// Set the web controller secret
    Secret {
        /// Secret key for authentication
        secret: String,
    },
}
