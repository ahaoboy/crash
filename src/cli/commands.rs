// Command handler implementations

use crate::cli::{Cli, Commands, ConfigCommands, InstallCommands, UpgradeRepo};
use crate::config::CrashConfig;
use crate::error::CrashError;
use crate::log_info;
use crate::utils::command::execute;
use crate::utils::monitor::format_status;
use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::{Shell, generate};
use std::io;

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
    easy_install::run_main(easy_install::Args::parse_from(v)).await
}

/// Handle start command
async fn handle_start(force: bool) -> Result<()> {
    log_info!("Executing start command");

    let mut config = CrashConfig::load()?;
    config.start(force).await?;
    println!("{} proxy service started successfully!", config.core);

    handle_status().await?;
    Ok(())
}

/// Handle stop command
async fn handle_stop(force: bool) -> Result<()> {
    log_info!("Executing stop command force: {}", force);

    let mut config = CrashConfig::load()?;
    config.stop(force)?;
    println!("{} proxy service stopped successfully!", config.core);

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
        return Err(CrashError::Platform("crontab not found".to_string()).into());
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

    for (cron, subcmd) in [("0 3 * * 3", "run-task"), ("*/10 * * * *", "start")] {
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
    ("CrashRunTask", "--schedule run-task", "WEEKLY", "WED", "03:00"),
        ("CrashStart", "--schedule start", "MINUTE", "", "00:00"),
    ];

    for (name, subcmd, schedule, days, time) in tasks {
        if execute("schtasks", &["/query", "/tn", name],  )
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

        if execute("schtasks", &args,  ).is_ok() {
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
        let status = execute("schtasks", &["/delete", "/tn", name, "/f"],  );
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

    for (cron, subcmd) in [("0 3 * * 3", "run-task"), ("*/10 * * * *", "start")] {
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

/// Handle config command and subcommands
fn handle_config(command: Option<ConfigCommands>) -> Result<()> {
    log_info!("Executing config command");

    match command {
        None => {
            // Show all config as JSON
            let config = CrashConfig::load()?;
            let json = serde_json::to_string_pretty(&config)?;
            println!("{}", json);
        }
        Some(ConfigCommands::Url { value }) => {
            let mut config = CrashConfig::load()?;
            match value {
                Some(url) => {
                    config.url = url.clone();
                    config.save()?;
                    println!("Configuration URL set to: {}", url);
                }
                None => {
                    println!("{}", config.url);
                }
            }
        }
        Some(ConfigCommands::Proxy { value }) => {
            let mut config = CrashConfig::load()?;
            match value {
                Some(proxy) => {
                    config.proxy = proxy;
                    config.save()?;
                    println!("Proxy set to: {}", proxy);
                }
                None => {
                    println!("{}", config.proxy);
                }
            }
        }
        Some(ConfigCommands::Ui { value }) => {
            let mut config = CrashConfig::load()?;
            match value {
                Some(ui) => {
                    config.web.ui = ui;
                    config.save()?;
                    println!("Web UI set to: {}", ui);
                }
                None => {
                    println!("{}", config.web.ui);
                }
            }
        }
        Some(ConfigCommands::Target { value }) => {
            let mut config = CrashConfig::load()?;
            match value {
                Some(target) => {
                    config.target = target;
                    config.save()?;
                    println!("Target set to: {}", target);
                }
                None => {
                    println!("{}", config.target);
                }
            }
        }
        Some(ConfigCommands::Host { value }) => {
            let mut config = CrashConfig::load()?;
            match value {
                Some(host) => {
                    config.web.host = host.clone();
                    config.save()?;
                    println!("Web host set to: {}", host);
                }
                None => {
                    println!("{}", config.web.host);
                }
            }
        }
        Some(ConfigCommands::Secret { value }) => {
            let mut config = CrashConfig::load()?;
            match value {
                Some(secret) => {
                    config.web.secret = secret;
                    config.save()?;
                    println!("Web secret updated successfully!");
                }
                None => {
                    println!("{}", config.web.secret);
                }
            }
        }
        Some(ConfigCommands::MaxRuntime { value }) => {
            let mut config = CrashConfig::load()?;
            match value {
                Some(hours) => {
                    config.max_runtime_hours = hours;
                    config.save()?;
                    if hours == 0 {
                        println!("Maximum runtime disabled (process will run indefinitely)");
                    } else {
                        println!("Maximum runtime set to {} hours", hours);
                    }
                }
                None => {
                    println!("{}", config.max_runtime_hours);
                }
            }
        }
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
