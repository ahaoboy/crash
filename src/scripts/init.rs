// Initialization module - corresponds to scripts/init.sh

use crate::common::{Config, Logger, Result, ShellCrashError, ShellExecutor};
use dialoguer::{Input, Select};
use std::fs;
use std::path::{Path, PathBuf};

pub const VERSION: &str = "1.9.2beta4";

pub struct InitManager {
    pub config: Config,
    shell: ShellExecutor,
    logger: Logger,
}

#[derive(Debug, Clone)]
pub enum SystemType {
    Padavan,
    AsusRouter,
    MiSnapshot,
    NgSnapshot,
    Generic,
}

impl InitManager {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            shell: ShellExecutor::new(),
            logger: Logger::new(),
        }
    }

    /// Detect system type
    pub fn detect_system_type(&self) -> SystemType {
        if Path::new("/etc/storage/started_script.sh").exists() {
            return SystemType::Padavan;
        }
        if Path::new("/jffs").is_dir() {
            return SystemType::AsusRouter;
        }
        if Path::new("/data/etc/crontabs/root").exists() {
            return SystemType::MiSnapshot;
        }
        if Path::new("/var/mnt/cfg/firewall").exists() {
            return SystemType::NgSnapshot;
        }
        SystemType::Generic
    }

    /// Set installation directory
    pub fn set_directory(&mut self) -> Result<PathBuf> {
        let systype = self.detect_system_type();

        let dir = match systype {
            SystemType::Padavan => PathBuf::from("/etc/storage"),
            SystemType::MiSnapshot => self.select_mi_directory()?,
            SystemType::AsusRouter => self.select_asus_directory()?,
            SystemType::NgSnapshot => PathBuf::from("/tmp/mnt"),
            SystemType::Generic => self.select_generic_directory()?,
        };

        // Check write permission
        if !self.check_write_permission(&dir) {
            return Err(ShellCrashError::PermissionDenied(format!(
                "没有{}目录写入权限！",
                dir.display()
            ))
            .into());
        }

        // Get available space
        let space = self.get_available_space(&dir)?;
        self.logger
            .info(&format!("目标目录 {} 空间剩余：{}", dir.display(), space));

        // Confirm installation
        let confirm: String = Input::new()
            .with_prompt("确认安装？(1/0)")
            .interact_text()
            .unwrap_or_else(|_| "0".to_string());

        if confirm == "1" {
            let crash_dir = dir.join("ShellCrash");
            self.config.crash_dir = crash_dir.clone();
            Ok(crash_dir)
        } else {
            self.set_directory()
        }
    }

    fn select_mi_directory(&self) -> Result<PathBuf> {
        self.logger
            .log_colored("检测到当前设备为小米官方系统，请选择安装位置", 33);

        let mut options = Vec::new();
        if self.get_available_space_mb(&PathBuf::from("/data"))? > 256 {
            options.push("安装到 /data 目录(推荐，支持软固化功能)");
        }
        if self.get_available_space_mb(&PathBuf::from("/userdisk"))? > 256 {
            options.push("安装到 /userdisk 目录(推荐，支持软固化功能)");
        }
        options.push("安装自定义目录(不推荐，不明勿用！)");
        options.push("退出安装");

        let selection = Select::new()
            .with_prompt("请选择")
            .items(&options)
            .interact()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        match selection {
            0 => Ok(PathBuf::from("/data")),
            1 => Ok(PathBuf::from("/userdisk")),
            2 => self.select_custom_directory(),
            _ => Err(ShellCrashError::Unknown("安装已取消".to_string()).into()),
        }
    }

    fn select_asus_directory(&self) -> Result<PathBuf> {
        self.logger
            .log_colored("检测到当前设备为华硕固件，请选择安装方式", 33);

        let options = vec![
            "基于USB设备安装(限23年9月之前固件，须插入任意USB设备)",
            "基于自启脚本安装(仅支持梅林及部分非koolshare官改固件)",
            "基于U盘+下载大师安装(支持所有固件，限ARM设备，须插入U盘或移动硬盘)",
            "退出安装",
        ];

        let selection = Select::new()
            .with_prompt("请选择")
            .items(&options)
            .interact()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        match selection {
            0 | 1 => Ok(PathBuf::from("/jffs")),
            2 => {
                self.logger.info("请先在路由器网页后台安装下载大师并启用");
                self.select_usb_directory()
            }
            _ => Err(ShellCrashError::Unknown("安装已取消".to_string()).into()),
        }
    }

    fn select_generic_directory(&self) -> Result<PathBuf> {
        self.logger
            .log_colored("安装ShellCrash至少需要预留约1MB的磁盘空间", 33);

        let options = vec![
            "在 /etc 目录下安装(适合root用户)",
            "在 /usr/share 目录下安装(适合Linux系统)",
            "在当前用户目录下安装(适合非root用户)",
            "在外置存储中安装",
            "手动设置安装目录",
            "退出安装",
        ];

        let selection = Select::new()
            .with_prompt("请选择")
            .items(&options)
            .interact()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        match selection {
            0 => Ok(PathBuf::from("/etc")),
            1 => Ok(PathBuf::from("/usr/share")),
            2 => {
                let home = dirs::home_dir()
                    .ok_or_else(|| ShellCrashError::PathNotFound("无法获取用户目录".to_string()))?;
                Ok(home.join(".local/share"))
            }
            3 => self.select_usb_directory(),
            4 => self.select_custom_directory(),
            _ => Err(ShellCrashError::Unknown("安装已取消".to_string()).into()),
        }
    }

    fn select_usb_directory(&self) -> Result<PathBuf> {
        // List available mount points
        let output = self
            .shell
            .execute("du -hL /mnt 2>/dev/null || du -hL /tmp/mnt 2>/dev/null")?;
        let mounts = String::from_utf8_lossy(&output.stdout);

        println!("可用挂载点：");
        println!("{}", mounts);

        let path: String = Input::new()
            .with_prompt("请输入挂载点路径")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        Ok(PathBuf::from(path))
    }

    fn select_custom_directory(&self) -> Result<PathBuf> {
        println!("-----------------------------------------------");
        println!("可用路径 剩余空间:");
        let _ = self.shell.execute("df -h | awk '{print $6,$4}' | sed 1d");
        println!(
            "路径是必须带 / 的格式，注意写入虚拟内存(/tmp,/opt,/sys...)的文件会在重启后消失！！！"
        );

        let path: String = Input::new()
            .with_prompt("请输入自定义路径")
            .interact_text()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let path = PathBuf::from(path);
        if !path.exists()
            || path.to_str().is_some_and(|s| {
                s.contains("tmp") || s.contains("opt") || s.contains("sys")
            })
        {
            self.logger.error("路径错误！请重新设置！");
            return self.select_custom_directory();
        }

        Ok(path)
    }

    /// Initialize configuration files
    pub fn initialize_config(&self) -> Result<()> {
        let crash_dir = &self.config.crash_dir;

        // Create directory structure
        fs::create_dir_all(crash_dir)?;
        fs::create_dir_all(crash_dir.join("configs"))?;
        fs::create_dir_all(crash_dir.join("yamls"))?;
        fs::create_dir_all(crash_dir.join("jsons"))?;
        fs::create_dir_all(crash_dir.join("tools"))?;
        fs::create_dir_all(crash_dir.join("task"))?;

        // Create config file if not exists
        let config_file = crash_dir.join("configs/ShellCrash.cfg");
        if !config_file.exists() {
            fs::write(&config_file, "#ShellCrash配置文件，不明勿动！\n")?;
        }

        // Create command.env file
        let env_file = crash_dir.join("configs/command.env");
        if !env_file.exists() {
            let tmp_dir = "/tmp/ShellCrash";
            let bin_dir = crash_dir.to_str().unwrap_or("");
            let content = format!(
                "TMPDIR={}\nBINDIR={}\nCOMMAND=\"$TMPDIR/CrashCore -d $BINDIR -f $TMPDIR/config.yaml\"\n",
                tmp_dir, bin_dir
            );
            fs::write(&env_file, content)?;
        }

        self.logger.info("配置文件初始化完成");
        Ok(())
    }

    /// Setup environment variables
    pub fn setup_environment(&self) -> Result<()> {
        let crash_dir = &self.config.crash_dir;
        let crash_dir_str = crash_dir.to_str().unwrap_or("");

        // Determine shell type
        let shell_type = if self.shell.check_command_exists("bash") {
            "bash"
        } else if self.shell.check_command_exists("ash") {
            "ash"
        } else {
            "sh"
        };

        // Find profile file
        let mut profile_paths = vec![
            PathBuf::from("/opt/etc/profile"),
            PathBuf::from("/jffs/configs/profile.add"),
        ];

        if let Some(home) = dirs::home_dir() {
            profile_paths.push(home.join(".bashrc"));
        }

        profile_paths.push(PathBuf::from("/etc/profile"));

        let profile = profile_paths
            .into_iter()
            .find(|p| p.exists() && self.check_write_permission(p));

        if let Some(profile_path) = profile {
            // Read existing content
            let content = fs::read_to_string(&profile_path).unwrap_or_default();

            // Remove old aliases
            let mut lines: Vec<String> = content
                .lines()
                .filter(|l| {
                    !l.contains("alias crash=")
                        && !l.contains("alias clash=")
                        && !l.contains("export CRASHDIR=")
                })
                .map(|s| s.to_string())
                .collect();

            // Add new aliases
            lines.push(format!(
                "alias crash=\"{} {}/menu.sh\"",
                shell_type, crash_dir_str
            ));
            lines.push(format!(
                "alias clash=\"{} {}/menu.sh\"",
                shell_type, crash_dir_str
            ));
            lines.push(format!("export CRASHDIR=\"{}\"", crash_dir_str));

            fs::write(&profile_path, lines.join("\n"))?;
            self.logger.info("环境变量设置完成");
        } else {
            self.logger.warn("无法写入环境变量！请检查安装权限！");
        }

        // Create /usr/bin/crash if possible
        let crash_bin = PathBuf::from("/usr/bin/crash");
        if self.check_write_permission(crash_bin.parent().unwrap_or(Path::new("/"))) {
            let content = format!(
                "#!/bin/{}\n{}/menu.sh $1 $2 $3 $4 $5\n",
                shell_type, crash_dir_str
            );
            fs::write(&crash_bin, content)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&crash_bin)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&crash_bin, perms)?;
            }
        }

        Ok(())
    }

    /// Setup firewall rules
    pub fn setup_firewall(&mut self) -> Result<()> {
        // Detect firewall type
        let firewall_mod = if self.shell.execute("iptables -j REDIRECT -h").is_ok() {
            "iptables"
        } else if self.shell.execute("nft add table inet shellcrash").is_ok() {
            "nftables"
        } else {
            "unknown"
        };

        self.config.set_value("firewall_mod", firewall_mod)?;
        self.logger.info(&format!("防火墙类型: {}", firewall_mod));

        Ok(())
    }

    // Helper methods
    fn check_write_permission(&self, path: &Path) -> bool {
        if let Some(parent) = path.parent()
            && parent.exists() {
                return fs::metadata(parent)
                    .map(|m| !m.permissions().readonly())
                    .unwrap_or(false);
            }
        false
    }

    fn get_available_space(&self, path: &Path) -> Result<String> {
        let output = self.shell.execute(&format!(
            "df -h {} | tail -1 | awk '{{print $4}}'",
            path.display()
        ))?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn get_available_space_mb(&self, path: &Path) -> Result<u64> {
        let output = self.shell.execute(&format!(
            "df {} | tail -1 | awk '{{print $4}}'",
            path.display()
        ))?;
        let space_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(space_str.parse().unwrap_or(0) / 1024)
    }
}
