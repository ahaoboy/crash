// CLI module for command-line interface

use crate::config::{core::Core, web::UiType};
use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use github_proxy::Proxy;
use guess_target::Target;
use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr};
pub mod commands;
pub mod output;

const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const GIT_HASH: &str = git_version::git_version!();
const VERSION: &str = const_str::concat!(CARGO_PKG_VERSION, " ", GIT_HASH);

/// Main CLI structure
#[derive(Parser, Clone, Debug)]
#[command(name = "crash", version=VERSION)]
#[command(about = "A tool for managing proxy cores like Clash/Mihomo/SingBox", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    EnumString,
    IntoStaticStr,
    Serialize,
    Deserialize,
    ValueEnum,
)]
pub enum UpgradeRepo {
    Crash,
    #[default]
    CrashAssets,
}

impl std::fmt::Display for UpgradeRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpgradeRepo::Crash => write!(f, "crash"),
            UpgradeRepo::CrashAssets => write!(f, "crash-assets"),
        }
    }
}

/// Install subcommands
#[derive(Subcommand, Clone, Debug)]
pub enum InstallCommands {
    /// Install proxy core
    Core,
    /// Install web UI
    Ui,
    /// Install GeoIP databases
    Geo,
    /// Install scheduled tasks
    Task,
}

/// Config subcommands
#[derive(Subcommand, Clone, Debug)]
pub enum ConfigCommands {
    /// Get or set the GitHub proxy for downloads
    Proxy {
        /// Proxy type (e.g., Direct, Ghproxy, etc.), omit to show current value
        #[arg(ignore_case = true)]
        value: Option<Proxy>,
    },
    /// Get or set the web UI type
    Ui {
        /// UI type (Metacubexd, Zashboard, Yacd), omit to show current value
        #[arg(ignore_case = true)]
        value: Option<UiType>,
    },
    /// Get or set the target platform
    Target {
        /// Target platform, omit to show current value
        #[arg(ignore_case = true)]
        value: Option<Target>,
    },
    /// Get or set the web controller host
    Host {
        /// Host address (e.g., :9090), omit to show current value
        value: Option<String>,
    },
    /// Get or set the web controller secret
    Secret {
        /// Secret key for authentication, omit to show current value
        value: Option<String>,
    },
    /// Get or set maximum runtime in hours (0 = disabled)
    MaxRuntime {
        /// Maximum runtime in hours (0 to disable), omit to show current value
        value: Option<u64>,
    },
}

/// Available CLI commands
#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    /// Install proxy core and UI components
    Install {
        /// Force reinstallation even if already installed (applies to all when no subcommand)
        #[arg(short, long, default_value_t = false)]
        force: bool,

        #[command(subcommand)]
        command: Option<InstallCommands>,
    },

    /// Start the proxy service
    Start {
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },

    /// Stop the proxy service
    Stop {
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },

    Core {
        #[arg(ignore_case = true)]
        core: Core,
    },

    /// Show service status
    Status,

    /// Run scheduled update task
    RunTask,

    RemoveTask,

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

    /// View or modify configuration
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommands>,
    },

    /// Upgrade crash to the latest version
    Upgrade {
        #[arg(default_value_t = UpgradeRepo::CrashAssets, ignore_case = true)]
        repo: UpgradeRepo,
    },

    #[command(trailing_var_arg = true, allow_hyphen_values = true)]
    Ei {
        args: Vec<String>,
    },

    /// Generate shell completion scripts
    Completions {
        /// Shell type (bash, zsh, fish, powershell, elvish)
        #[arg(ignore_case = true)]
        shell: Shell,
    },
}
