// Command handler implementations

use crate::cli::Commands;
use crate::config::ConfigHandle;
use crate::core::CoreManager;
use crate::error::{CrashError, Result};
use crate::process::ProcessMonitor;
use crate::log_info;
use github_proxy::Proxy;
use std::str::FromStr;

/// Command handler for executing CLI commands
pub struct CommandHandler {
    config: ConfigHandle,
    core_manager: CoreManager,
    monitor: ProcessMonitor,
}

impl CommandHandler {
    /// Create a new command handler
    pub fn new(config: ConfigHandle) -> Self {
        let core_manager = CoreManager::new(config.clone());

        Self {
            config,
            core_manager,
            monitor: ProcessMonitor::new(),
        }
    }

    /// Handle a CLI command
    pub async fn handle(&mut self, command: Option<Commands>) -> Result<()> {
        match command {
            Some(Commands::Install { force }) => self.handle_install(force).await,
            Some(Commands::Proxy { proxy }) => self.handle_proxy(proxy),
            Some(Commands::Start) => self.handle_start(),
            Some(Commands::Stop) => self.handle_stop(),
            Some(Commands::Restart) => self.handle_restart(),
            Some(Commands::Status) => self.handle_status(),
            Some(Commands::Task) => self.handle_task(),
            Some(Commands::RunTask) => self.handle_run_task().await,
            Some(Commands::Url { url }) => self.handle_url(url),
            Some(Commands::UpdateUrl { force }) => self.handle_update_url(force).await,
            Some(Commands::UpdateGeo { force }) => self.handle_update_geo(force).await,
            Some(Commands::Update) => self.handle_update().await,
            Some(Commands::Ui { ui }) => self.handle_ui(ui),
            Some(Commands::Host { host }) => self.handle_host(host),
            Some(Commands::Secret { secret }) => self.handle_secret(secret),
            None => self.handle_status(),
        }
    }

    /// Handle install command
    async fn handle_install(&mut self, force: bool) -> Result<()> {
        log_info!("Executing install command (force: {})", force);

        self.core_manager.install(force).await?;

        println!("Installation completed successfully!");
        Ok(())
    }

    /// Handle proxy command
    fn handle_proxy(&self, proxy: Proxy) -> Result<()> {
        log_info!("Setting proxy to: {}", proxy);

        let mut config = self.config.write().map_err(|_| {
            CrashError::Config("Failed to acquire write lock on config".to_string())
        })?;

        config.proxy = proxy;
        config.save()?;

        println!("Proxy set to: {}", config.proxy);
        Ok(())
    }

    /// Handle start command
    fn handle_start(&mut self) -> Result<()> {
        log_info!("Executing start command");

        self.core_manager.start()?;

        println!("Proxy service started successfully!");
        Ok(())
    }

    /// Handle stop command
    fn handle_stop(&mut self) -> Result<()> {
        log_info!("Executing stop command");

        self.core_manager.stop()?;

        println!("Proxy service stopped successfully!");
        Ok(())
    }

    /// Handle restart command
    fn handle_restart(&mut self) -> Result<()> {
        log_info!("Executing restart command");

        self.core_manager.restart()?;

        println!("Proxy service restarted successfully!");
        Ok(())
    }

    /// Handle status command
    fn handle_status(&self) -> Result<()> {
        log_info!("Executing status command");

        let config = self
            .config
            .read()
            .map_err(|_| CrashError::Config("Failed to acquire read lock on config".to_string()))?;

        let exe_name = config.core.exe_name();
        let is_running = self.core_manager.is_running(&exe_name);
        let pid = if is_running {
            self.core_manager.get_pid(&exe_name).ok()
        } else {
            None
        };

        let status = self.monitor.format_status(&config, is_running, pid);
        println!("{}", status);

        Ok(())
    }

    /// Handle task command (install cron task)
    fn handle_task(&self) -> Result<()> {
        log_info!("Executing task command");

        #[cfg(unix)]
        {
            use crate::platform::command::CommandExecutor;

            let exe = std::env::current_exe().map_err(|e| {
                CrashError::Platform(format!("Failed to get current executable path: {}", e))
            })?;

            let exe_path = exe.to_string_lossy();
            let cmd = format!("{} run-task", exe_path);
            let cron = "0 3 * * 3"; // Every Wednesday at 3 AM
            let entry = format!("{} {}", cron, cmd);

            let executor = CommandExecutor;

            // Check if entry already exists
            if let Ok(list) = executor.execute("crontab", &["-l"]) {
                if list.lines().any(|line| line == entry) {
                    println!("Scheduled task already exists");
                    return Ok(());
                }
            }

            // Add cron entry
            let sh = format!("(crontab -l 2>/dev/null; echo '{}') | crontab -", entry);
            executor.execute("bash", &["-c", &sh])?;

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
    async fn handle_run_task(&mut self) -> Result<()> {
        log_info!("Executing run-task command");

        // Update configuration
        self.handle_update_url(true).await?;

        // Update geo databases
        self.handle_update_geo(true).await?;

        // Restart service
        self.handle_restart()?;

        println!("Scheduled task completed successfully!");
        Ok(())
    }

    /// Handle url command
    fn handle_url(&self, url: String) -> Result<()> {
        log_info!("Setting configuration URL to: {}", url);

        let mut config = self.config.write().map_err(|_| {
            CrashError::Config("Failed to acquire write lock on config".to_string())
        })?;

        config.url = url.clone();
        config.save()?;

        println!("Configuration URL set to: {}", url);
        Ok(())
    }

    /// Handle update-url command
    async fn handle_update_url(&self, force: bool) -> Result<()> {
        log_info!("Updating configuration from URL (force: {})", force);

        let (url, dest) = {
            let config = self
                .config
                .read()
                .map_err(|_| CrashError::Config("Failed to acquire read lock on config".to_string()))?;

            if config.url.is_empty() {
                return Err(CrashError::Config(
                    "Configuration URL not set. Use 'url' command first.".to_string(),
                ));
            }

            (config.url.clone(), config.config_path())
        }; // Lock is dropped here

        self.core_manager
            .updater()
            .update_config(&url, &dest, force)
            .await?;

        println!("Configuration updated successfully!");
        Ok(())
    }

    /// Handle update-geo command
    async fn handle_update_geo(&self, force: bool) -> Result<()> {
        log_info!("Updating GeoIP databases (force: {})", force);

        let config_clone = {
            let config = self
                .config
                .read()
                .map_err(|_| CrashError::Config("Failed to acquire read lock on config".to_string()))?;

            config.clone()
        }; // Lock is dropped here

        self.core_manager
            .updater()
            .update_geo(&config_clone, force)
            .await?;

        println!("GeoIP databases updated successfully!");
        Ok(())
    }

    /// Handle update command
    async fn handle_update(&self) -> Result<()> {
        log_info!("Updating configuration from stored URL");

        self.handle_update_url(false).await
    }

    /// Handle ui command
    fn handle_ui(&self, ui: String) -> Result<()> {
        log_info!("Setting UI to: {}", ui);

        use crate::config::web::UiType;

        let ui_type = UiType::from_str(&ui).map_err(|_| {
            CrashError::Config(format!(
                "Invalid UI type: {}. Valid options: Metacubexd, Zashboard, Yacd",
                ui
            ))
        })?;

        let mut config = self.config.write().map_err(|_| {
            CrashError::Config("Failed to acquire write lock on config".to_string())
        })?;

        config.web.ui = ui_type;
        config.save()?;

        println!("Web UI set to: {}", ui);
        Ok(())
    }

    /// Handle host command
    fn handle_host(&self, host: String) -> Result<()> {
        log_info!("Setting web host to: {}", host);

        let mut config = self.config.write().map_err(|_| {
            CrashError::Config("Failed to acquire write lock on config".to_string())
        })?;

        config.web.host = host.clone();
        config.save()?;

        println!("Web host set to: {}", host);
        Ok(())
    }

    /// Handle secret command
    fn handle_secret(&self, secret: String) -> Result<()> {
        log_info!("Setting web secret");

        let mut config = self.config.write().map_err(|_| {
            CrashError::Config("Failed to acquire write lock on config".to_string())
        })?;

        config.web.secret = secret;
        config.save()?;

        println!("Web secret updated successfully!");
        Ok(())
    }
}
