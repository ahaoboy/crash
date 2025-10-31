use clap::{Parser, Subcommand};
use crash::core::{APP_CONFIG, UI, app_config_dir, mkdir};
use github_proxy::Proxy;

#[derive(Parser)]
#[command(name = "crash", version)]
#[command(about = "crash - A tool for managing proxy cores like Clash/Mihomo/SingBox", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Install {
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },

    Proxy {
        proxy: Proxy,
    },

    /// Start the service
    Start,

    /// Stop the service
    Stop,

    /// Restart the service
    Restart,

    /// Show service status
    Status,

    /// Manage tasks
    Task,

    RunTask,

    Url {
        url: String,
    },

    UpdateUrl {
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },
    UpdateGeo {
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },

    Update,

    Ui {
        ui: UI,
    },
    Host {
        host: String,
    },
    Secret {
        secret: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    mkdir(app_config_dir().as_str());
    let cli = Cli::parse();
    // Handle commands
    match cli.command {
        Some(Commands::Url { url }) => {
            let mut config = APP_CONFIG
                .write()
                .map_err(|_| anyhow::anyhow!("Failed to acquire write lock for app config"))?;
            config.url = url;
            config.save()?;
            Ok(())
        }
        Some(Commands::Install { force }) => {
            let config = {
                APP_CONFIG
                    .read()
                    .map_err(|_| anyhow::anyhow!("Failed to read app config"))?
            };
            config.install(force).await;
            config.update_geo(force).await?;
            Ok(())
        }
        Some(Commands::Proxy { proxy }) => {
            let mut config = APP_CONFIG
                .write()
                .map_err(|_| anyhow::anyhow!("Failed to acquire write lock for app config"))?;
            config.proxy = proxy;
            println!("Proxy set to {}", config.proxy);
            config.save()?;
            Ok(())
        }
        Some(Commands::UpdateUrl { force }) => {
            let mut config = APP_CONFIG
                .write()
                .map_err(|_| anyhow::anyhow!("Failed to acquire write lock for app config"))?;
            config.update_url(force).await?;
            config.restart()?;
            Ok(())
        }
        Some(Commands::UpdateGeo { force }) => {
            let mut config = APP_CONFIG
                .write()
                .map_err(|_| anyhow::anyhow!("Failed to acquire write lock for app config"))?;
            config.update_geo(force).await?;
            config.restart()?;
            Ok(())
        }
        Some(Commands::Start) => {
            let mut config = APP_CONFIG
                .write()
                .map_err(|_| anyhow::anyhow!("Failed to read app config"))?;
            config.start()?;
            Ok(())
        }
        Some(Commands::RunTask) => {
            let mut config = APP_CONFIG
                .write()
                .map_err(|_| anyhow::anyhow!("Failed to read app config"))?;
            config.update_url(true).await?;
            config.update_geo(true).await?;
            config.restart()?;
            Ok(())
        }
        Some(Commands::Stop) => {
            let mut config = APP_CONFIG
                .write()
                .map_err(|_| anyhow::anyhow!("Failed to read app config"))?;
            config.stop()?;
            Ok(())
        }
        Some(Commands::Restart) => {
            let mut config = APP_CONFIG
                .write()
                .map_err(|_| anyhow::anyhow!("Failed to read app config"))?;
            config.restart()?;
            Ok(())
        }

        Some(Commands::Task) => {
            let config = APP_CONFIG
                .read()
                .map_err(|_| anyhow::anyhow!("Failed to read app config"))?;
            config.install_task()?;
            Ok(())
        }
        Some(Commands::Update) => {
            let config = APP_CONFIG
                .read()
                .map_err(|_| anyhow::anyhow!("Failed to read app config"))?;

            config.update(&config.url).await;
            Ok(())
        }
        Some(Commands::Ui { ui }) => {
            let mut config = APP_CONFIG
                .write()
                .map_err(|_| anyhow::anyhow!("Failed to acquire write lock for app config"))?;
            config.web.ui = ui;

            Ok(())
        }
        Some(Commands::Host { host }) => {
            let mut config = APP_CONFIG
                .write()
                .map_err(|_| anyhow::anyhow!("Failed to acquire write lock for app config"))?;
            config.web.host = host;

            Ok(())
        }
        Some(Commands::Secret { secret }) => {
            let mut config = APP_CONFIG
                .write()
                .map_err(|_| anyhow::anyhow!("Failed to acquire write lock for app config"))?;
            config.web.secret = secret;

            Ok(())
        }
        None | Some(Commands::Status) => {
            let config = APP_CONFIG
                .read()
                .map_err(|_| anyhow::anyhow!("Failed to acquire write lock for app config"))?;
            let s = config.status();
            println!("{s}");
            Ok(())
        }
    }
}
