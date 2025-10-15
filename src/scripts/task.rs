// Task management - corresponds to scripts/task.sh

use crate::common::{Config, Logger, Result, ShellCrashError, ShellExecutor};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub struct TaskManager {
    config: Config,
    shell: ShellExecutor,
    logger: Logger,
    task_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: u32,
    pub name: String,
    pub command: String,
    pub schedule: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub enum TaskType {
    Cron(String),       // Cron expression
    BeforeStart,        // Run before service starts
    AfterStart,         // Run after service starts
    Running(String),    // Run while service is running
    AfterFirewall,      // Run after firewall setup
}

impl TaskManager {
    pub fn new(config: Config) -> Self {
        let task_file = config.crash_dir.join("task/task.list");
        Self {
            config,
            shell: ShellExecutor::new(),
            logger: Logger::new(),
            task_file,
        }
    }

    /// Add a new task
    pub fn add_task(&mut self, task: Task, task_type: TaskType) -> Result<()> {
        self.logger.info(&format!("添加任务: {}", task.name));

        // Add to crontab or task file based on type
        match task_type {
            TaskType::Cron(cron_expr) => {
                self.add_cron_task(&task, &cron_expr)?;
            }
            TaskType::BeforeStart => {
                self.add_service_task(&task, "bfstart")?;
            }
            TaskType::AfterStart => {
                self.add_service_task(&task, "afstart")?;
            }
            TaskType::Running(interval) => {
                self.add_running_task(&task, &interval)?;
            }
            TaskType::AfterFirewall => {
                self.add_service_task(&task, "affirewall")?;
            }
        }

        self.logger.info("任务添加成功");
        Ok(())
    }

    /// Remove a task
    pub fn remove_task(&mut self, task_id: u32) -> Result<()> {
        self.logger.info(&format!("删除任务 ID: {}", task_id));

        // Remove from crontab
        self.remove_from_cron(task_id)?;

        // Remove from service task files
        for file in &["bfstart", "afstart", "running", "affirewall"] {
            let path = self.config.crash_dir.join(format!("task/{}", file));
            if path.exists() {
                let content = fs::read_to_string(&path)?;
                let filtered: Vec<&str> = content
                    .lines()
                    .filter(|line| !line.contains(&format!("task.sh {}", task_id)))
                    .collect();
                fs::write(&path, filtered.join("\n"))?;
            }
        }

        self.logger.info("任务删除成功");
        Ok(())
    }

    /// List all tasks
    pub fn list_tasks(&self) -> Vec<Task> {
        let mut tasks = Vec::new();

        // Read from task list file
        if let Ok(content) = fs::read_to_string(&self.task_file) {
            for line in content.lines() {
                if line.starts_with('#') || line.trim().is_empty() {
                    continue;
                }

                let parts: Vec<&str> = line.split('#').collect();
                if parts.len() >= 3 {
                    if let Ok(id) = parts[0].trim().parse::<u32>() {
                        tasks.push(Task {
                            id,
                            command: parts[1].trim().to_string(),
                            name: parts[2].trim().to_string(),
                            schedule: String::new(),
                            description: String::new(),
                        });
                    }
                }
            }
        }

        // Read custom tasks
        let custom_file = self.config.crash_dir.join("task/task.user");
        if let Ok(content) = fs::read_to_string(custom_file) {
            for line in content.lines() {
                if line.starts_with('#') || line.trim().is_empty() {
                    continue;
                }

                let parts: Vec<&str> = line.split('#').collect();
                if parts.len() >= 3 {
                    if let Ok(id) = parts[0].trim().parse::<u32>() {
                        tasks.push(Task {
                            id,
                            command: parts[1].trim().to_string(),
                            name: parts[2].trim().to_string(),
                            schedule: String::new(),
                            description: String::new(),
                        });
                    }
                }
            }
        }

        tasks
    }

    /// Execute a task
    pub fn execute_task(&self, task_id: u32) -> Result<()> {
        let tasks = self.list_tasks();
        let task = tasks
            .iter()
            .find(|t| t.id == task_id)
            .ok_or_else(|| ShellCrashError::Unknown(format!("任务 {} 不存在", task_id)))?;

        self.logger.info(&format!("执行任务: {}", task.name));

        match self.shell.execute(&task.command) {
            Ok(_) => {
                self.logger.info(&format!("任务【{}】执行成功", task.name));
                Ok(())
            }
            Err(e) => {
                self.logger
                    .error(&format!("任务【{}】执行失败: {}", task.name, e));
                Err(e)
            }
        }
    }

    /// Update core (auto-update task)
    pub fn update_core(&self) -> Result<()> {
        use crate::scripts::Downloader;

        self.logger.info("开始更新内核...");

        let downloader = Downloader::new(self.config.clone());
        downloader.update_core()?;

        self.logger.info("内核更新完成");
        Ok(())
    }

    /// Update scripts
    pub fn update_scripts(&self) -> Result<()> {
        use crate::scripts::Downloader;

        self.logger.info("开始更新脚本...");

        let downloader = Downloader::new(self.config.clone());
        downloader.update_scripts()?;

        self.logger.info("脚本更新完成");
        Ok(())
    }

    /// Update GeoIP database
    pub fn update_geoip(&self) -> Result<()> {
        use crate::scripts::Downloader;

        self.logger.info("开始更新 GeoIP 数据库...");

        let downloader = Downloader::new(self.config.clone());
        downloader.update_geoip()?;

        self.logger.info("GeoIP 数据库更新完成");
        Ok(())
    }

    // Helper methods
    fn add_cron_task(&self, task: &Task, cron_expr: &str) -> Result<()> {
        let crash_dir = self.config.crash_dir.to_str().unwrap_or("");
        let cron_line = format!(
            "{} {}/task/task.sh {} {}",
            cron_expr, crash_dir, task.id, task.name
        );

        // Get current crontab
        let current_cron = self
            .shell
            .execute("crontab -l 2>/dev/null")
            .unwrap_or_else(|_| std::process::Output {
                status: std::process::ExitStatus::default(),
                stdout: Vec::new(),
                stderr: Vec::new(),
            });

        let mut cron_content = String::from_utf8_lossy(&current_cron.stdout).to_string();

        // Remove old entry if exists
        cron_content = cron_content
            .lines()
            .filter(|line| !line.contains(&format!("task.sh {}", task.id)))
            .collect::<Vec<_>>()
            .join("\n");

        // Add new entry
        cron_content.push_str(&format!("\n{}\n", cron_line));

        // Write back
        let tmp_file = self.config.tmp_dir.join("cron_tmp");
        fs::write(&tmp_file, cron_content)?;
        self.shell
            .execute(&format!("crontab {}", tmp_file.display()))?;
        fs::remove_file(tmp_file)?;

        Ok(())
    }

    fn add_service_task(&self, task: &Task, task_type: &str) -> Result<()> {
        let task_file = self.config.crash_dir.join(format!("task/{}", task_type));
        let crash_dir = self.config.crash_dir.to_str().unwrap_or("");

        let task_line = format!("{}/task/task.sh {} {}\n", crash_dir, task.id, task.name);

        // Append to file
        let mut content = if task_file.exists() {
            fs::read_to_string(&task_file)?
        } else {
            String::new()
        };

        // Remove old entry if exists
        content = content
            .lines()
            .filter(|line| !line.contains(&format!("task.sh {}", task.id)))
            .collect::<Vec<_>>()
            .join("\n");

        content.push_str(&task_line);
        fs::write(task_file, content)?;

        Ok(())
    }

    fn add_running_task(&self, task: &Task, interval: &str) -> Result<()> {
        // Parse interval to cron expression
        let cron_expr = if let Ok(minutes) = interval.parse::<u32>() {
            if minutes < 60 {
                format!("*/{} * * * *", minutes)
            } else {
                let hours = minutes / 60;
                format!("* */{} * * *", hours)
            }
        } else {
            return Err(ShellCrashError::ConfigError(
                "无效的时间间隔".to_string(),
            ).into());
        };

        self.add_service_task(task, "running")?;

        // Also add to crontab for running tasks
        self.add_cron_task(task, &cron_expr)?;

        Ok(())
    }

    fn remove_from_cron(&self, task_id: u32) -> Result<()> {
        let current_cron = self
            .shell
            .execute("crontab -l 2>/dev/null")
            .unwrap_or_else(|_| std::process::Output {
                status: std::process::ExitStatus::default(),
                stdout: Vec::new(),
                stderr: Vec::new(),
            });

        let cron_content = String::from_utf8_lossy(&current_cron.stdout);
        let filtered: Vec<&str> = cron_content
            .lines()
            .filter(|line| !line.contains(&format!("task.sh {}", task_id)))
            .collect();

        let tmp_file = self.config.tmp_dir.join("cron_tmp");
        fs::write(&tmp_file, filtered.join("\n"))?;
        self.shell
            .execute(&format!("crontab {}", tmp_file.display()))?;
        fs::remove_file(tmp_file)?;

        Ok(())
    }

    /// Show interactive task menu
    pub fn show_menu(&mut self) -> Result<()> {
        use dialoguer::{Input, Select};

        loop {
            println!("-----------------------------------------------");
            println!("任务管理");
            println!("-----------------------------------------------");

            let tasks = self.list_tasks();
            if !tasks.is_empty() {
                println!("当前任务列表:");
                for task in &tasks {
                    println!(" {} - {}", task.id, task.name);
                }
                println!("-----------------------------------------------");
            }

            let options = vec![
                "1 添加任务",
                "2 删除任务",
                "3 执行任务",
                "4 查看任务日志",
                "0 返回上级菜单",
            ];

            let selection = Select::new()
                .with_prompt("请选择")
                .items(&options)
                .interact()
                .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

            match selection {
                0 => {
                    // Add task
                    self.add_task_interactive()?;
                }
                1 => {
                    // Remove task
                    let task_id: String = Input::new()
                        .with_prompt("请输入要删除的任务ID")
                        .interact_text()
                        .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

                    if let Ok(id) = task_id.parse::<u32>() {
                        self.remove_task(id)?;
                    } else {
                        self.logger.error("无效的任务ID");
                    }
                }
                2 => {
                    // Run task
                    let task_id: String = Input::new()
                        .with_prompt("请输入要执行的任务ID")
                        .interact_text()
                        .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

                    if let Ok(id) = task_id.parse::<u32>() {
                        self.run_task(id)?;
                    } else {
                        self.logger.error("无效的任务ID");
                    }
                }
                3 => {
                    // View logs
                    self.view_task_logs()?;
                }
                4 => break,
                _ => {}
            }
        }

        Ok(())
    }

    /// Add task interactively
    pub fn add_task_interactive(&self) -> Result<()> {
        use dialoguer::{Input, Select};

        println!("-----------------------------------------------");
        println!("添加任务");

        let task_types = vec![
            "自动更新内核",
            "自动更新脚本",
            "自动更新数据库",
            "重设防火墙",
            "自定义命令",
        ];

        let task_type_idx = Select::new()
            .with_prompt("请选择任务类型")
            .items(&task_types)
            .interact()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let (command, name) = match task_type_idx {
            0 => ("update_core".to_string(), "自动更新内核".to_string()),
            1 => ("update_scripts".to_string(), "自动更新脚本".to_string()),
            2 => ("update_geoip".to_string(), "自动更新数据库".to_string()),
            3 => ("reset_firewall".to_string(), "重设防火墙".to_string()),
            4 => {
                let cmd: String = Input::new()
                    .with_prompt("请输入命令")
                    .interact_text()
                    .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;
                let name: String = Input::new()
                    .with_prompt("请输入任务名称")
                    .interact_text()
                    .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;
                (cmd, name)
            }
            _ => return Ok(()),
        };

        let schedule_types = vec![
            "每周执行",
            "每日执行",
            "每小时执行",
            "每分钟执行",
            "服务启动前执行",
            "服务启动后执行",
            "服务运行时执行",
        ];

        let schedule_idx = Select::new()
            .with_prompt("请选择执行时间")
            .items(&schedule_types)
            .interact()
            .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;

        let task_type = match schedule_idx {
            0 => {
                let day: String = Input::new()
                    .with_prompt("在每周哪天执行？(0-6, 0=周日)")
                    .interact_text()
                    .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;
                let hour: String = Input::new()
                    .with_prompt("在哪个小时执行？(0-23)")
                    .interact_text()
                    .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;
                TaskType::Cron(format!("0 {} * * {}", hour, day))
            }
            1 => {
                let hour: String = Input::new()
                    .with_prompt("在哪个小时执行？(0-23)")
                    .interact_text()
                    .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;
                TaskType::Cron(format!("0 {} * * *", hour))
            }
            2 => {
                let interval: String = Input::new()
                    .with_prompt("每隔多少小时执行？(1-23)")
                    .interact_text()
                    .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;
                TaskType::Cron(format!("0 */{} * * *", interval))
            }
            3 => {
                let interval: String = Input::new()
                    .with_prompt("每隔多少分钟执行？(1-59)")
                    .interact_text()
                    .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;
                TaskType::Cron(format!("*/{} * * * *", interval))
            }
            4 => TaskType::BeforeStart,
            5 => TaskType::AfterStart,
            6 => {
                let interval: String = Input::new()
                    .with_prompt("每隔多少分钟执行？(1-1440)")
                    .interact_text()
                    .map_err(|e| ShellCrashError::Unknown(e.to_string()))?;
                TaskType::Running(interval)
            }
            _ => return Ok(()),
        };

        // Generate task ID
        let tasks = self.list_tasks();
        let max_id = tasks.iter().map(|t| t.id).max().unwrap_or(100);
        let task_id = max_id + 1;

        let task = Task {
            id: task_id,
            name: name.clone(),
            command,
            schedule: String::new(),
            description: String::new(),
        };

        let mut task_manager = TaskManager::new(self.config.clone());
        task_manager.add_task(task, task_type)?;

        Ok(())
    }

    /// Run a task by ID
    pub fn run_task(&self, task_id: u32) -> Result<()> {
        self.execute_task(task_id)
    }

    /// View task logs
    fn view_task_logs(&self) -> Result<()> {
        let log_file = self.config.tmp_dir.join("ShellCrash.log");
        if log_file.exists() {
            let content = fs::read_to_string(log_file)?;
            let task_logs: Vec<&str> = content
                .lines()
                .filter(|line| line.contains("任务"))
                .collect();

            if task_logs.is_empty() {
                println!("没有找到任务日志");
            } else {
                println!("-----------------------------------------------");
                println!("任务执行日志:");
                for log in task_logs {
                    println!("{}", log);
                }
            }
        } else {
            println!("日志文件不存在");
        }

        Ok(())
    }
}
