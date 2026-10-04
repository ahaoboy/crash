// Runtime operations on the proxy core: start / stop / version probing.
//
// These are split out of `config/mod.rs` so that the storage / validation
// code in `mod.rs` stays small, while process-lifecycle concerns live here.

use super::CrashConfig;
use super::core::Core;
use super::get_config_dir;
use crate::error::{CrashError, Result};
use crate::utils::check_connectivity;
use crate::utils::command::execute;
use crate::utils::current_timestamp;
use crate::utils::process::{get_pid, start, stop};
use crate::utils::tun::{ensure_tun_device, tun_enabled_json, tun_enabled_yaml};
use crate::{log_debug, log_info, log_warn};

impl CrashConfig {
    /// Start the proxy core, restarting it first if `force` or if the runtime
    /// budget has been exceeded.
    pub async fn start(&mut self, force: bool) -> Result<()> {
        log_info!("Starting proxy core: {}", self.core.name());

        if self.stop_force {
            if !force {
                return Err(CrashError::Process(
                    "Skip starting proxy core: run 'crash start -f' instead.".to_string(),
                ));
            } else {
                self.stop_force = false;
                self.save()?;
            }
        }

        if get_pid(&self.core.exe_name()).is_ok() {
            let current_time = current_timestamp();
            let runtime_seconds = current_time.saturating_sub(self.start_time);
            let max_runtime_seconds = self.max_runtime_hours * 3600;

            let exceeds_runtime = self.max_runtime_hours > 0
                && self.start_time > 0
                && runtime_seconds >= max_runtime_seconds;

            // Health check: under TUN + auto-route (the mihomo default),
            // this request is captured by the TUN device and forwarded
            // through the proxy, so a failure means the proxy is not
            // forwarding correctly — restart it to reload the config.
            let check_url = self
                .check_url
                .as_deref()
                .unwrap_or("https://www.google.com");
            let connectivity_ok = check_connectivity(check_url).await;

            let needs_restart = force || exceeds_runtime || !connectivity_ok;

            if needs_restart {
                let reason = if force {
                    "manual force stop"
                } else if exceeds_runtime {
                    "maximum runtime exceeded"
                } else {
                    "connectivity check failed"
                };
                log_info!("Stopping process, reason: {}", reason);
                self.stop(false)?;
            } else {
                return Ok(());
            }
        }

        // TUN device prep is only meaningful when the active core config
        // enables TUN; skip it entirely otherwise. Failures inside are logged
        // and never abort the start.
        if self.tun_enabled() {
            ensure_tun_device();
        } else {
            log_info!("TUN not enabled in core config, skipping TUN device setup");
        }

        self.start_core()?;
        self.start_time = current_timestamp();
        self.save()?;

        log_info!("Proxy core started successfully");
        Ok(())
    }

    /// Whether the active core's configuration enables TUN.
    ///
    /// Gates the TUN device preparation: without TUN there is no point
    /// touching the `tun` kernel module or `/dev/net/tun`. A config that is
    /// missing or cannot be parsed is treated as TUN-disabled, so this check
    /// can never block a start on its own.
    pub fn tun_enabled(&self) -> bool {
        let path = self.core_config_path();
        let Ok(config) = std::fs::read_to_string(&path) else {
            log_warn!(
                "Failed to read core config {}, assuming TUN disabled",
                path.display()
            );
            return false;
        };

        let enabled = match self.core {
            Core::Mihomo | Core::Clash => tun_enabled_yaml(&config),
            Core::Singbox => tun_enabled_json(&config),
        };

        log_debug!("TUN enabled in {}: {}", path.display(), enabled);
        enabled
    }

    /// Spawn the core executable with the right arguments for the current core.
    pub fn start_core(&self) -> Result<()> {
        let exe_path = self.core.exe_path(&get_config_dir());

        if !exe_path.exists() {
            return Err(CrashError::Process(format!(
                "Core executable not found: {}. Please run 'install' first.",
                exe_path.display()
            )));
        }

        let args = match self.core {
            Core::Mihomo | Core::Clash => vec![
                "-f".to_string(),
                self.core_config_path().to_string_lossy().to_string(),
                "-ext-ctl".to_string(),
                self.web.host.clone(),
                "-ext-ui".to_string(),
                self.web.ui_name().to_string(),
                "-d".to_string(),
                get_config_dir().to_string_lossy().to_string(),
            ],
            Core::Singbox => vec![
                "run".to_string(),
                "-c".to_string(),
                self.core_config_path().to_string_lossy().to_string(),
                "-D".to_string(),
                get_config_dir().to_string_lossy().to_string(),
            ],
        };

        start(&exe_path, args, self.core.envs())?;
        Ok(())
    }

    /// Stop the proxy core.
    pub fn stop(&mut self, force: bool) -> Result<()> {
        log_info!("Stopping proxy core: {}", self.core.name());

        self.stop_force = force;
        let exe_name = self.core.exe_name();
        stop(&exe_name)?;

        self.start_time = 0;
        self.save()?;

        log_info!("Proxy core stopped successfully");
        Ok(())
    }

    /// Get the version of the installed proxy core.
    pub fn get_version(&self) -> Result<String> {
        log_debug!("Getting version for core: {}", self.core.name());

        let exe_path = self.core.exe_path(&get_config_dir());

        if !exe_path.exists() {
            log_debug!("Core executable not found: {}", exe_path.display());
            return Err(CrashError::Config("Core executable not found".to_string()));
        }

        let args = match self.core {
            Core::Mihomo | Core::Clash => &["-v"],
            Core::Singbox => &["version"],
        };
        let output = execute(exe_path.to_string_lossy().as_ref(), args)?;

        // Parse version from output (format: "Mihomo version 1.19.15")
        let Some(version) = output.split_whitespace().nth(2).map(|s| s.to_string()) else {
            return Err(CrashError::Config("Core version not found".to_string()));
        };

        log_debug!("Core version: {:?}", version);
        Ok(version)
    }
}
