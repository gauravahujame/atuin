use eyre::{Context, Result};
#[cfg(windows)]
use tokio::net::TcpStream;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;

use hyper_util::rt::TokioIo;

#[cfg(unix)]
use tokio::net::UnixStream;

use atuin_client::history::History;

use crate::history::{
    EndHistoryRequest, StartHistoryRequest, history_client::HistoryClient as HistoryServiceClient,
};

pub struct HistoryClient {
    client: HistoryServiceClient<Channel>,
}

// Wrap the grpc client
impl HistoryClient {
    /// Maximum number of connection retry attempts
    const MAX_RETRIES: u32 = 3;
    /// Initial backoff duration in milliseconds
    const INITIAL_BACKOFF_MS: u64 = 100;

    #[cfg(unix)]
    async fn try_connect(path: &str) -> Result<Channel> {
        let path = path.to_owned();
        Endpoint::try_from("http://atuin_local_daemon:0")?
            .connect_with_connector(service_fn(move |_: Uri| {
                let path = path.clone();
                async move {
                    Ok::<_, std::io::Error>(TokioIo::new(UnixStream::connect(path.clone()).await?))
                }
            }))
            .await
            .map_err(Into::into)
    }

    #[cfg(unix)]
    pub async fn new(path: String) -> Result<Self> {
        use std::time::Duration;

        let mut retries = 0;
        let mut backoff = Duration::from_millis(Self::INITIAL_BACKOFF_MS);

        loop {
            match Self::try_connect(&path).await {
                Ok(channel) => {
                    if retries > 0 {
                        tracing::info!(
                            "connected to daemon after {} retries",
                            retries
                        );
                    }
                    let client = HistoryServiceClient::new(channel);
                    return Ok(HistoryClient { client });
                }
                Err(e) if retries < Self::MAX_RETRIES => {
                    tracing::warn!(
                        "connection to daemon at {} failed (attempt {}/{}), retrying in {:?}: {}",
                        &path,
                        retries + 1,
                        Self::MAX_RETRIES + 1,
                        backoff,
                        e
                    );
                    tokio::time::sleep(backoff).await;
                    backoff *= 2;
                    retries += 1;
                }
                Err(e) => {
                    return Err(e).wrap_err_with(|| format!(
                        "failed to connect to atuin daemon at {} after {} attempts. \
                         The daemon may not be running. Try: atuin daemon",
                        &path, Self::MAX_RETRIES + 1
                    ));
                }
            }
        }
    }

    #[cfg(not(unix))]
    async fn try_connect_tcp(port: u64) -> Result<Channel> {
        let url = format!("127.0.0.1:{}", port);
        Endpoint::try_from("http://atuin_local_daemon:0")?
            .connect_with_connector(service_fn(move |_: Uri| {
                let url = url.clone();
                async move {
                    Ok::<_, std::io::Error>(TokioIo::new(TcpStream::connect(url.clone()).await?))
                }
            }))
            .await
            .map_err(Into::into)
    }

    #[cfg(not(unix))]
    pub async fn new(port: u64) -> Result<Self> {
        use std::time::Duration;

        let mut retries = 0;
        let mut backoff = Duration::from_millis(Self::INITIAL_BACKOFF_MS);

        loop {
            match Self::try_connect_tcp(port).await {
                Ok(channel) => {
                    if retries > 0 {
                        tracing::info!(
                            "connected to daemon after {} retries",
                            retries
                        );
                    }
                    let client = HistoryServiceClient::new(channel);
                    return Ok(HistoryClient { client });
                }
                Err(e) if retries < Self::MAX_RETRIES => {
                    tracing::warn!(
                        "connection to daemon at 127.0.0.1:{} failed (attempt {}/{}), retrying in {:?}: {}",
                        port,
                        retries + 1,
                        Self::MAX_RETRIES + 1,
                        backoff,
                        e
                    );
                    tokio::time::sleep(backoff).await;
                    backoff *= 2;
                    retries += 1;
                }
                Err(e) => {
                    return Err(e).wrap_err_with(|| format!(
                        "failed to connect to atuin daemon at 127.0.0.1:{} after {} attempts. \
                         The daemon may not be running. Try: atuin daemon",
                        port, Self::MAX_RETRIES + 1
                    ));
                }
            }
        }
    }

    pub async fn start_history(&mut self, h: History) -> Result<String> {
        let req = StartHistoryRequest {
            command: h.command,
            cwd: h.cwd,
            hostname: h.hostname,
            session: h.session,
            timestamp: h.timestamp.unix_timestamp_nanos() as u64,
        };

        let resp = self.client.start_history(req).await?;

        Ok(resp.into_inner().id)
    }

    pub async fn end_history(
        &mut self,
        id: String,
        duration: u64,
        exit: i64,
    ) -> Result<(String, u64)> {
        let req = EndHistoryRequest { id, duration, exit };

        let resp = self.client.end_history(req).await?;
        let resp = resp.into_inner();

        Ok((resp.id, resp.idx))
    }
}

/// Health check result from the daemon
#[derive(Debug)]
pub struct DaemonHealthInfo {
    pub healthy: bool,
    pub version: String,
    pub uptime_seconds: u64,
    pub running_commands: u32,
}

/// Status result from check_daemon_health
#[derive(Debug)]
pub enum DaemonStatus {
    /// Daemon is running and healthy
    Running(DaemonHealthInfo),
    /// Socket file does not exist - daemon not started
    NotRunning,
    /// Socket exists but daemon is not responding - stale socket
    StaleSocket,
    /// Connection failed with an error
    Error(String),
}

use crate::history::{health_client::HealthClient as HealthServiceClient, HealthCheckRequest};

/// Check the health status of the daemon
/// Returns a DaemonStatus indicating whether the daemon is running, stopped, stale, or errored
#[cfg(unix)]
pub async fn check_daemon_health(socket_path: &str) -> DaemonStatus {
    use std::path::Path;

    // Check if socket file exists
    if !Path::new(socket_path).exists() {
        return DaemonStatus::NotRunning;
    }

    // Try to connect
    let path = socket_path.to_owned();
    let channel = match Endpoint::try_from("http://atuin_local_daemon:0") {
        Ok(ep) => {
            ep.connect_with_connector(service_fn(move |_: Uri| {
                let path = path.clone();
                async move {
                    Ok::<_, std::io::Error>(TokioIo::new(UnixStream::connect(path.clone()).await?))
                }
            }))
            .await
            .ok()
        }
        Err(_) => None,
    };

    let channel = match channel {
        Some(ch) => ch,
        None => return DaemonStatus::StaleSocket,
    };

    // Call health check
    let mut client = HealthServiceClient::new(channel);
    match client.check(HealthCheckRequest {}).await {
        Ok(response) => {
            let reply = response.into_inner();
            DaemonStatus::Running(DaemonHealthInfo {
                healthy: reply.healthy,
                version: reply.version,
                uptime_seconds: reply.uptime_seconds,
                running_commands: reply.running_commands,
            })
        }
        Err(e) => DaemonStatus::Error(e.to_string()),
    }
}

/// Check the health status of the daemon (Windows/TCP version)
#[cfg(not(unix))]
pub async fn check_daemon_health(port: u64) -> DaemonStatus {
    let url = format!("127.0.0.1:{}", port);

    // Try to connect
    let channel = match Endpoint::try_from("http://atuin_local_daemon:0") {
        Ok(ep) => {
            let url = url.clone();
            ep.connect_with_connector(service_fn(move |_: Uri| {
                let url = url.clone();
                async move {
                    Ok::<_, std::io::Error>(TokioIo::new(TcpStream::connect(url.clone()).await?))
                }
            }))
            .await
            .ok()
        }
        Err(_) => None,
    };

    let channel = match channel {
        Some(ch) => ch,
        None => return DaemonStatus::NotRunning,
    };

    // Call health check
    let mut client = HealthServiceClient::new(channel);
    match client.check(HealthCheckRequest {}).await {
        Ok(response) => {
            let reply = response.into_inner();
            DaemonStatus::Running(DaemonHealthInfo {
                healthy: reply.healthy,
                version: reply.version,
                uptime_seconds: reply.uptime_seconds,
                running_commands: reply.running_commands,
            })
        }
        Err(e) => DaemonStatus::Error(e.to_string()),
    }
}

