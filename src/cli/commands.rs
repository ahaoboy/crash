// Command handler implementations

use crate::cli::Commands;
use crate::config::CrashConfig;
use crate::core::updater::{update_config, update_geo};
use crate::error::{CrashError, Result};
use crate::log_info;
use crate::process::monitor::format_status;
use github_proxy::Proxy;
use std::str::FromStr;

pub async fn handle(command: Option<Commands>) -> Result<()> {
    match command {
        Some(Commands::Install { force }) => handle_install(force).await,
        Some(Commands::Proxy { proxy }) => handle_proxy(proxy),
        Some(Commands::Start) => handle_start(),
        Some(Commands::Stop) => handle_stop(),
        Some(Commands::Restart) => handle_restart(),
        Some(Commands::Status) => handle_status(),
        Some(Commands::Task) => handle_task(),
        Some(Commands::RunTask) => handle_run_task().await,
        Some(Commands::Url { url }) => handle_url(url),
        Some(Commands::UpdateUrl { force }) => handle_update_url(force).await,
        Some(Commands::UpdateGeo { force }) => handle_update_geo(force).await,
        Some(Commands::Update) => handle_update().await,
        Some(Commands::Ui { ui }) => handle_ui(ui),
        Some(Commands::Host { host }) => handle_host(host),
        Some(Commands::Secret { secret }) => handle_secret(secret),
        None => handle_status(),
    }
}

/// Handle install command
async fn handle_install(force: bool) -> Result<()> {
    log_info!("Executing install command (force: {})", force);

    CrashConfig::load()?.install(force).await?;

    println!("Installation completed successfully!");
    Ok(())
}

/// Handle proxy command
fn handle_proxy(proxy: Proxy) -> Result<()> {
    log_info!("Setting proxy to: {}", proxy);
    let mut config = CrashConfig::load()?;

    config.proxy = proxy;
    config.save()?;

    println!("Proxy set to: {}", config.proxy);
    Ok(())
}

/// Handle start command
fn handle_start() -> Result<()> {
    log_info!("Executing start command");

    CrashConfig::load()?.start()?;

    println!("Proxy service started successfully!");
    Ok(())
}

/// Handle stop command
fn handle_stop() -> Result<()> {
    log_info!("Executing stop command");

    CrashConfig::load()?.stop()?;

    println!("Proxy service stopped successfully!");
    Ok(())
}

/// Handle restart command
fn handle_restart() -> Result<()> {
    log_info!("Executing restart command");

    CrashConfig::load()?.restart()?;

    println!("Proxy service restarted successfully!");
    Ok(())
}

/// Handle status command
fn handle_status() -> Result<()> {
    log_info!("Executing status command");
    let config = CrashConfig::load()?;
    let status = format_status(&config);
    println!("{}", status);
    Ok(())
}

/// Handle task command (install cron task)
fn handle_task() -> Result<()> {
    log_info!("Executing task command");

    #[cfg(unix)]
    {
        use crate::platform::command::execute;

        let exe = std::env::current_exe().map_err(|e| {
            CrashError::Platform(format!("Failed to get current executable path: {}", e))
        })?;

        let exe_path = exe.to_string_lossy();
        let cmd = format!("{} run-task", exe_path);
        let cron = "0 3 * * 3"; // Every Wednesday at 3 AM
        let entry = format!("{} {}", cron, cmd);

        // Check if entry already exists
        if let Ok(list) = execute("crontab", &["-l"]) {
            if list.lines().any(|line| line == entry) {
                println!("Scheduled task already exists");
                return Ok(());
            }
        }

        // Add cron entry
        let sh = format!("(crontab -l 2>/dev/null; echo '{}') | crontab -", entry);
        execute("bash", &["-c", &sh])?;

        println!("Scheduled task installed successfully!");
        println!("Task will run: {}", cron);
    }

    #[cfg(windows)]
    {
        println!("Scheduled tasks are not supported on Windows yet");
    }

    Ok(())
}

/// Handle run-task command
async fn handle_run_task() -> Result<()> {
    log_info!("Executing run-task command");

    // Update configuration
    handle_update_url(true).await?;

    // Update geo databases
    handle_update_geo(true).await?;

    // Restart service
    handle_restart()?;

    println!("Scheduled task completed successfully!");
    Ok(())
}

/// Handle url command
fn handle_url(url: String) -> Result<()> {
    log_info!("Setting configuration URL to: {}", url);

    let mut config = CrashConfig::load()?;

    config.url = url.clone();
    config.save()?;

    println!("Configuration URL set to: {}", url);
    Ok(())
}

/// Handle update-url command
async fn handle_update_url(force: bool) -> Result<()> {
    log_info!("Updating configuration from URL (force: {})", force);

    let (url, dest) = {
        let config = CrashConfig::load()?;
        if config.url.is_empty() {
            return Err(CrashError::Config(
                "Configuration URL not set. Use 'url' command first.".to_string(),
            ));
        }

        (config.url.clone(), config.config_path())
    }; // Lock is dropped here

    update_config(&url, &dest, force).await?;

    println!("Configuration updated successfully!");
    Ok(())
}

/// Handle update-geo command
async fn handle_update_geo(force: bool) -> Result<()> {
    log_info!("Updating GeoIP databases (force: {})", force);

    let config_clone = CrashConfig::load()?;

    update_geo(&config_clone, force).await?;

    println!("GeoIP databases updated successfully!");
    Ok(())
}

/// Handle update command
async fn handle_update() -> Result<()> {
    log_info!("Updating configuration from stored URL");

    handle_update_url(false).await
}

/// Handle ui command
fn handle_ui(ui: String) -> Result<()> {
    log_info!("Setting UI to: {}", ui);

    use crate::config::web::UiType;

    let ui_type = UiType::from_str(&ui).map_err(|_| {
        CrashError::Config(format!(
            "Invalid UI type: {}. Valid options: Metacubexd, Zashboard, Yacd",
            ui
        ))
    })?;

    let mut config = CrashConfig::load()?;
    config.web.ui = ui_type;
    config.save()?;

    println!("Web UI set to: {}", ui);
    Ok(())
}

/// Handle host command
fn handle_host(host: String) -> Result<()> {
    log_info!("Setting web host to: {}", host);

    let mut config = CrashConfig::load()?;

    config.web.host = host.clone();
    config.save()?;

    println!("Web host set to: {}", host);
    Ok(())
}

/// Handle secret command
fn handle_secret(secret: String) -> Result<()> {
    log_info!("Setting web secret");

    let mut config = CrashConfig::load()?;

    config.web.secret = secret;
    config.save()?;

    println!("Web secret updated successfully!");
    Ok(())
}
