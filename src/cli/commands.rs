// Command handler implementations

use crate::cli::{Cli, Commands, ConfigCommands, InstallCommands, UpgradeRepo};
use crate::config::CrashConfig;
use crate::error::{CrashError, Result};
use crate::log_info;
use crate::utils::command::execute;
use crate::utils::monitor::format_status;
use clap::{CommandFactory, Parser};
use clap_complete::{Shell, generate};
use std::io;
use std::time::Duration;

pub async fn handle(command: Option<Commands>) -> Result<()> {
    match command {
        Some(Commands::Install { force, command }) => handle_install(force, command).await,
        Some(Commands::Start { force }) => handle_start(force).await,
        Some(Commands::Stop { force }) => handle_stop(force).await,
        Some(Commands::Status) => handle_status().await,
        Some(Commands::RunTask) => handle_run_task().await,
        Some(Commands::RemoveTask) => handle_remove_task(),
        Some(Commands::UpdateUrl { force }) => handle_update_url(force).await,
        Some(Commands::Config { command }) => handle_config(command),
        Some(Commands::Upgrade { repo }) => handle_upgrade(repo).await,
        Some(Commands::Ei { args }) => handle_ei(args).await,
        Some(Commands::Completions { shell }) => handle_completions(shell),
        None => handle_status().await,
    }
}

/// Handle install command
async fn handle_install(force: bool, command: Option<InstallCommands>) -> Result<()> {
    log_info!(
        "Executing install command (force: {}, subcommand: {:?})",
        force,
        command
    );

    let config = CrashConfig::load()?;

    match command {
        Some(InstallCommands::Core) => {
            config.install_core(force).await?;
            println!("Core installation completed successfully!");
        }
        Some(InstallCommands::Ui) => {
            config.install_ui(force).await?;
            println!("UI installation completed successfully!");
        }
        Some(InstallCommands::Geo) => {
            config.install_geo(force).await?;
            println!("Geo installation completed successfully!");
        }
        Some(InstallCommands::Task) => {
            handle_task()?;
            println!("Task installation completed successfully!");
        }
        None => {
            // Install all components
            config.install(force).await?;
            handle_task()?;
            println!("Installation completed successfully!");
        }
    }

    Ok(())
}

async fn handle_ei(args: Vec<String>) -> Result<()> {
    log_info!("Executing ei command (args: {:?})", args);
    let mut v = vec!["ei".to_string()];
    v.extend(args);
    easy_install::run_main(easy_install::Args::parse_from(v))
        .await
        .map_err(|e| CrashError::External(e.to_string()))
}

/// Handle start command
async fn handle_start(force: bool) -> Result<()> {
    log_info!("Executing start command");

    let mut config = CrashConfig::load()?;
    config.start(force).await?;
    println!("{} proxy service started successfully!", config.core);

    tokio::time::sleep(Duration::from_secs_f32(0.1)).await;
    handle_status().await?;

    Ok(())
}

/// Handle stop command
async fn handle_stop(force: bool) -> Result<()> {
    log_info!("Executing stop command force: {}", force);

    let mut config = CrashConfig::load()?;
    config.stop(force)?;
    println!("{} proxy service stopped successfully!", config.core);

    tokio::time::sleep(Duration::from_secs_f32(0.1)).await;
    handle_status().await?;

    Ok(())
}

/// Handle status command
async fn handle_status() -> Result<()> {
    log_info!("Executing status command");
    let config = CrashConfig::load()?;
    let status = format_status(&config).await;
    println!("{}", status);
    Ok(())
}

/// Cron schedule entries installed on Unix systems: (cron expression, crash subcommand).
#[cfg(unix)]
const UNIX_SCHEDULES: [(&str, &str); 2] = [("0 3 * * 3", "run-task"), ("*/10 * * * *", "start")];

#[cfg(unix)]
fn handle_task() -> Result<()> {
    use which::which;

    use crate::utils::get_user;

    log_info!("Executing task command");

    let exe = std::env::current_exe().map_err(|e| {
        CrashError::Platform(format!("Failed to get current executable path: {}", e))
    })?;

    let exe_path = exe.to_string_lossy();

    if which("crontab").is_err() {
        return Err(CrashError::Platform("crontab not found".to_string()));
    }

    let user = get_user();
    for d in [
        "/etc/storage/cron/crontabs",
        "/var/spool/cron/crontabs",
        "/var/spool/cron",
    ] {
        let p = format!("{}/{}", d, user);
        if std::fs::exists(d).unwrap_or(false)
            && !std::fs::exists(&p).unwrap_or(false)
            && std::fs::write(p, "").is_ok()
        {
            break;
        }
    }

    for (cron, subcmd) in UNIX_SCHEDULES {
        let cmd = format!("{} {}", exe_path, subcmd);
        let entry = format!("{} {}", cron, cmd);

        if let Ok(list) = execute("crontab", &["-l"]) {
            if list.lines().any(|line| line == entry) {
                println!("Scheduled task already exists");
                continue;
            }

            let sh = format!("(crontab -l 2>/dev/null; echo '{}') | crontab -", entry);
            execute("bash", &["-c", &sh])?;
            println!("Scheduled task installed successfully!");
            println!("Task will run: {}", cron);
        }
    }

    Ok(())
}

#[cfg(windows)]
fn handle_task() -> Result<()> {
    log_info!("Executing task command");

    let exe = std::env::current_exe().map_err(|e| {
        CrashError::Platform(format!("Failed to get current executable path: {}", e))
    })?;

    let exe_path = exe.to_string_lossy();

    let tasks = [
        (
            "CrashRunTask",
            "--schedule run-task",
            "WEEKLY",
            "WED",
            "03:00",
        ),
        ("CrashStart", "--schedule start", "MINUTE", "", "00:00"),
    ];

    for (name, subcmd, schedule, days, time) in tasks {
        if execute("schtasks", &["/query", "/tn", name])
            .unwrap_or_default()
            .contains(name)
        {
            continue;
        }

        let full_cmd = format!("\"{}\" {}", exe_path, subcmd);

        let mut args = vec!["/create", "/tn", name, "/tr", &full_cmd, "/sc", schedule];

        if !days.is_empty() {
            args.extend_from_slice(&["/d", days]);
        }

        if schedule.eq_ignore_ascii_case("MINUTE") {
            args.extend_from_slice(&["/mo", "10"]);
        }

        args.extend_from_slice(&["/st", time]);

        args.extend_from_slice(&["/rl", "LIMITED"]);

        if execute("schtasks", &args).is_ok() {
            println!("Scheduled task '{}' created successfully.", name);
        } else {
            println!("Scheduled task '{}' created error.", name);
        }
    }

    Ok(())
}

#[cfg(windows)]
fn handle_remove_task() -> Result<()> {
    println!("Removing Windows scheduled task");
    for name in ["CrashRunTask", "CrashStart"] {
        let status = execute("schtasks", &["/delete", "/tn", name, "/f"]);
        if status.is_ok() {
            println!("Task '{}' deleted successfully.", name);
        } else {
            println!("Task '{}' deleted error.", name);
        }
    }
    Ok(())
}

#[cfg(unix)]
pub fn handle_remove_task() -> Result<()> {
    println!("Removing Unix scheduled task");

    let current = execute("crontab", &["-l"])?;
    let mut new_lines = Vec::new();

    let exe = std::env::current_exe().map_err(|e| {
        CrashError::Platform(format!("Failed to get current executable path: {}", e))
    })?;

    let exe_path = exe.to_string_lossy();

    for (cron, subcmd) in UNIX_SCHEDULES {
        let cmd = format!("{} {}", exe_path, subcmd);
        let entry = format!("{} {}", cron, cmd);

        for line in current.lines() {
            if !line.contains(&entry) {
                new_lines.push(line);
            } else {
                println!("Removed: {}", line);
            }
        }
    }

    let mut child = std::process::Command::new("crontab")
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        std::io::Write::write_all(stdin, new_lines.join("\n").as_bytes())?;
    }

    let status = child.wait()?;
    if status.success() {
        println!("Cron task removed successfully.");
    } else {
        println!("Cron task removed error.");
    }
    Ok(())
}

/// Handle run-task command
async fn handle_run_task() -> Result<()> {
    log_info!("Executing run-task command");

    // Update configuration
    handle_update_url(true).await?;

    // Update geo databases
    // handle_update_geo(true).await?;

    handle_start(true).await?;

    println!("Scheduled task completed successfully!");
    Ok(())
}

/// Handle update-url command
async fn handle_update_url(force: bool) -> Result<()> {
    let config = CrashConfig::load()?;
    log_info!(
        "Updating {} configuration from URL (force: {})",
        config.core,
        force
    );

    config.update_config(force).await?;

    println!("{} configuration updated successfully!", config.core);
    Ok(())
}

async fn handle_upgrade(repo: UpgradeRepo) -> Result<()> {
    log_info!("Executing upgrade command");

    let config = CrashConfig::load()?;
    config.upgrade(repo).await?;

    Ok(())
}

/// Load the config, apply a mutation, save it, and print the message returned
/// by the closure. Centralises the load/save/print boilerplate that every
/// `config <field> <value>` subcommand would otherwise repeat.
fn mutate_config<F: FnOnce(&mut CrashConfig) -> String>(f: F) -> Result<()> {
    let mut config = CrashConfig::load()?;
    let msg = f(&mut config);
    config.save()?;
    println!("{}", msg);
    Ok(())
}

/// Handle config command and subcommands
fn handle_config(command: Option<ConfigCommands>) -> Result<()> {
    log_info!("Executing config command");

    match command {
        None => {
            let config = CrashConfig::load()?;
            let json = serde_json::to_string_pretty(&config)?;
            println!("{}", json);
        }
        Some(ConfigCommands::Url { value }) => match value {
            Some(url) => mutate_config(|c| {
                c.url = url;
                format!("Configuration URL set to: {}", c.url)
            })?,
            None => println!("{}", CrashConfig::load()?.url),
        },
        Some(ConfigCommands::Proxy { value }) => match value {
            Some(proxy) => mutate_config(|c| {
                c.proxy = proxy;
                format!("Proxy set to: {}", c.proxy)
            })?,
            None => println!("{}", CrashConfig::load()?.proxy),
        },
        Some(ConfigCommands::Ui { value }) => match value {
            Some(ui) => mutate_config(|c| {
                c.web.ui = ui;
                format!("Web UI set to: {}", c.web.ui)
            })?,
            None => println!("{}", CrashConfig::load()?.web.ui),
        },
        Some(ConfigCommands::Target { value }) => match value {
            Some(target) => mutate_config(|c| {
                c.target = target;
                format!("Target set to: {}", c.target)
            })?,
            None => println!("{}", CrashConfig::load()?.target),
        },
        Some(ConfigCommands::Host { value }) => match value {
            Some(host) => mutate_config(|c| {
                c.web.host = host;
                format!("Web host set to: {}", c.web.host)
            })?,
            None => println!("{}", CrashConfig::load()?.web.host),
        },
        Some(ConfigCommands::Secret { value }) => match value {
            Some(secret) => mutate_config(|c| {
                c.web.secret = secret;
                "Web secret updated successfully!".to_string()
            })?,
            None => println!("{}", CrashConfig::load()?.web.secret),
        },
        Some(ConfigCommands::MaxRuntime { value }) => match value {
            Some(hours) => mutate_config(|c| {
                c.max_runtime_hours = hours;
                if hours == 0 {
                    "Maximum runtime disabled (process will run indefinitely)".to_string()
                } else {
                    format!("Maximum runtime set to {} hours", hours)
                }
            })?,
            None => println!("{}", CrashConfig::load()?.max_runtime_hours),
        },
    }

    Ok(())
}

/// Handle completions command
fn handle_completions(shell: Shell) -> Result<()> {
    log_info!("Generating completions for shell: {:?}", shell);

    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();

    generate(shell, &mut cmd, bin_name, &mut io::stdout());

    Ok(())
}
