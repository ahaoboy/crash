// ShellCrash Rust Implementation - Main Entry Point
// Copyright (C) Rust Port

use clap::{Parser, Subcommand};
use crash::Config;
use std::{path::PathBuf, str::FromStr};

// Re-export for convenience

#[derive(Parser)]
#[command(name = "crash", version)]
#[command(about = "crash - A tool for managing proxy cores like Clash/Mihomo/SingBox", long_about = None)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize ShellCrash
    Init,

    /// Show interactive menu
    Menu,

    /// Start the service
    Start,

    /// Stop the service
    Stop,

    /// Restart the service
    Restart,

    /// Show service status
    Status,

    /// Manage tasks
    Task {
        #[command(subcommand)]
        action: Option<TaskCommands>,
    },

    /// Manage DDNS
    Ddns {
        #[command(subcommand)]
        action: Option<DdnsCommands>,
    },

    /// Set language (en/zh)
    Lang {
        /// Language code: en (English) or zh (Chinese)
        language: String,
    },
}

#[derive(Subcommand)]
enum TaskCommands {
    /// List all tasks
    List,
    /// Add a new task
    Add,
    /// Remove a task
    Remove { id: u32 },
    /// Execute a task
    Run { id: u32 },
}

#[derive(Subcommand)]
enum DdnsCommands {
    /// List all DDNS services
    List,
    /// Add a new DDNS service
    Add,
    /// Remove a DDNS service
    Remove { name: String },
    /// Update a DDNS service
    Update { name: String },
}

fn main() -> anyhow::Result<()> {
    // Initialize logger
    env_logger::init();

    // Load language preference
    if let Some(config_dir) = dirs::config_dir() {
        let lang_file = config_dir.join("shellcrash").join("language");
        if let Ok(lang_code) = std::fs::read_to_string(&lang_file)
            && let Ok(lang) = crash::common::Language::from_str(lang_code.trim())
        {
            crash::common::set_language(lang);
        }
    }

    let cli = Cli::parse();

    // Load configuration
    let config = if let Some(config_path) = cli.config {
        Config::load(&config_path)?
    } else {
        // Try default locations
        let default_paths = vec![
            PathBuf::from("/etc/ShellCrash/configs/ShellCrash.cfg"),
            PathBuf::from("~/.local/share/ShellCrash/configs/ShellCrash.cfg"),
            PathBuf::from("./configs/ShellCrash.cfg"),
        ];

        let mut config = None;
        for path in default_paths {
            if path.exists() {
                config = Some(Config::load(&path)?);
                break;
            }
        }

        config.unwrap_or_default()
    };

    // Handle commands
    match cli.command {
        Some(Commands::Init) => {
            use crash::scripts::InitManager;

            println!("初始化 ShellCrash...");
            let mut init_manager = InitManager::new(config.clone());

            // Set installation directory
            let crash_dir = init_manager.set_directory()?;
            println!("安装目录: {}", crash_dir.display());

            // Initialize configuration
            init_manager.initialize_config()?;

            // Setup environment
            init_manager.setup_environment()?;

            // Setup firewall
            init_manager.setup_firewall()?;

            println!("\x1b[32m脚本初始化完成,请输入 crash 命令开始使用！\x1b[0m");
            Ok(())
        }
        Some(Commands::Menu) => {
            use crash::scripts::MenuSystem;

            let menu = MenuSystem::new(config);
            menu.show_main_menu()
        }
        Some(Commands::Start) => {
            use crash::scripts::ServiceManager;

            let service = ServiceManager::new(config);
            service.start()
        }
        Some(Commands::Stop) => {
            use crash::scripts::ServiceManager;

            let service = ServiceManager::new(config);
            service.stop()
        }
        Some(Commands::Restart) => {
            use crash::scripts::ServiceManager;

            let service = ServiceManager::new(config);
            service.restart()
        }
        Some(Commands::Status) => {
            use crash::scripts::ServiceManager;

            let service = ServiceManager::new(config);
            let status = service.get_status();

            match status {
                crash::scripts::menu::ServiceStatus::Running {
                    pid,
                    uptime,
                    memory,
                    mode,
                } => {
                    println!("服务状态: \x1b[32m运行中\x1b[0m");
                    println!("PID: {}", pid);
                    println!("运行模式: {}", mode);
                    println!("内存使用: {:.2} MB", memory as f64 / 1024.0);
                    println!("运行时长: {:?}", uptime);
                }
                crash::scripts::menu::ServiceStatus::Stopped => {
                    println!("服务状态: \x1b[31m已停止\x1b[0m");
                }
                crash::scripts::menu::ServiceStatus::Error(e) => {
                    println!("服务状态: \x1b[31m错误 - {}\x1b[0m", e);
                }
            }
            Ok(())
        }
        Some(Commands::Task { action }) => {
            use crash::scripts::TaskManager;

            let mut task_manager = TaskManager::new(config);

            match action {
                Some(TaskCommands::List) => {
                    let tasks = task_manager.list_tasks();
                    if tasks.is_empty() {
                        println!("没有任务");
                    } else {
                        println!("任务列表:");
                        for task in tasks {
                            println!(" {} - {}", task.id, task.name);
                        }
                    }
                    Ok(())
                }
                Some(TaskCommands::Add) => task_manager.add_task_interactive(),
                Some(TaskCommands::Remove { id }) => task_manager.remove_task(id),
                Some(TaskCommands::Run { id }) => task_manager.run_task(id),
                None => {
                    println!("请指定任务操作");
                    println!("使用 --help 查看可用命令");
                    Ok(())
                }
            }
        }
        Some(Commands::Ddns { action }) => {
            use crash::tools::DDNSManager;

            let mut ddns_manager = DDNSManager::new(config);

            match action {
                Some(DdnsCommands::List) => {
                    let services = ddns_manager.list_services();
                    if services.is_empty() {
                        println!("没有 DDNS 服务");
                    } else {
                        println!("DDNS 服务列表:");
                        for service in services {
                            println!(
                                " {} - {} ({})",
                                service.name, service.domain, service.service_name
                            );
                        }
                    }
                    Ok(())
                }
                Some(DdnsCommands::Add) => ddns_manager.add_service_interactive(),
                Some(DdnsCommands::Remove { name }) => ddns_manager.remove_service(&name),
                Some(DdnsCommands::Update { name }) => ddns_manager.update_service(&name),
                None => {
                    println!("请指定 DDNS 操作");
                    println!("使用 --help 查看可用命令");
                    Ok(())
                }
            }
        }
        Some(Commands::Lang { language }) => {
            use crash::common::{Language, set_language};

            if let Ok(lang) = Language::from_str(&language) {
                set_language(lang);

                // Save language preference to config
                let lang_file = dirs::config_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("shellcrash")
                    .join("language");

                if let Some(parent) = lang_file.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&lang_file, lang.code());

                match lang {
                    Language::English => println!("Language changed to English"),
                    Language::Chinese => println!("语言已切换为中文"),
                }
            } else {
                eprintln!("Invalid language. Use 'en' for English or 'zh' for Chinese.");
            }
            Ok(())
        }
        None => {
            use crash::scripts::MenuSystem;

            // Default: show menu
            let menu = MenuSystem::new(config);
            menu.show_main_menu()
        }
    }
}
