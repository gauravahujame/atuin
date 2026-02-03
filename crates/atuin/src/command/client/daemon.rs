use clap::Subcommand;
use eyre::Result;

use atuin_client::{database::Sqlite, record::sqlite_store::SqliteStore, settings::Settings};
use atuin_daemon::server::listen;

#[derive(Subcommand, Debug)]
#[command(infer_subcommands = true)]
pub enum Cmd {
    /// Start the daemon (default)
    #[command()]
    Start,

    /// Check daemon status
    #[command()]
    Check,
}

impl Default for Cmd {
    fn default() -> Self {
        Self::Start
    }
}

impl Cmd {
    pub async fn run(
        self,
        settings: Settings,
        store: SqliteStore,
        history_db: Sqlite,
    ) -> Result<()> {
        match self {
            Self::Start => {
                listen(settings, store, history_db).await?;
            }
            Self::Check => {
                check_daemon_status(&settings).await?;
            }
        }
        Ok(())
    }
}

async fn check_daemon_status(settings: &Settings) -> Result<()> {
    use atuin_daemon::client::{check_daemon_health, DaemonStatus};

    #[cfg(unix)]
    let status = check_daemon_health(&settings.daemon.socket_path).await;

    #[cfg(not(unix))]
    let status = check_daemon_health(settings.daemon.tcp_port).await;

    match status {
        DaemonStatus::Running(info) => {
            println!("Daemon status: RUNNING");
            println!("  Version: {}", info.version);
            println!("  Uptime: {} seconds", info.uptime_seconds);
            println!("  Running commands: {}", info.running_commands);

            #[cfg(unix)]
            println!("  Socket: {}", settings.daemon.socket_path);
            #[cfg(not(unix))]
            println!("  Port: {}", settings.daemon.tcp_port);
        }
        DaemonStatus::NotRunning => {
            println!("Daemon status: NOT RUNNING");
            #[cfg(unix)]
            println!("Socket file does not exist at: {}", settings.daemon.socket_path);
            #[cfg(not(unix))]
            println!(
                "Could not connect to daemon at 127.0.0.1:{}",
                settings.daemon.tcp_port
            );

            println!("\nTo start the daemon, run: atuin daemon start");
        }
        DaemonStatus::StaleSocket => {
            #[cfg(unix)]
            {
                let socket_path = &settings.daemon.socket_path;
                println!("Daemon status: NOT RUNNING (stale socket)");
                println!(
                    "Socket file exists at {} but daemon is not responding.",
                    socket_path
                );
                println!("\nThe socket file is stale. You can:");
                println!("  1. Start the daemon: atuin daemon start");
                println!("  2. Or manually remove the stale socket: rm {}", socket_path);
            }
            #[cfg(not(unix))]
            println!("Daemon status: NOT RUNNING (connection refused)");
        }
        DaemonStatus::Error(e) => {
            println!("Daemon status: ERROR");
            println!("Failed to check daemon status: {}", e);
        }
    }

    Ok(())
}

