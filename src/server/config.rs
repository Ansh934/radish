use super::listener::Listener;

/// Default address for raddish server to listen on.
const DEFAULT_ADDR: &str = "127.0.0.1:6379";

/// Top-level server handle.
///
/// # Examples
/// ```no_run
/// // Start on the default address (127.0.0.1:6379)
/// Server::run().await?;
///
/// // Start on a custom address
/// Server::run_at("0.0.0.0:5379").await?;
/// ```
pub struct Server;

impl Server {
    /// Starts the server on the default address (`127.0.0.1:6379`).
    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        Self::run_at(DEFAULT_ADDR).await
    }

    /// Starts the server bound to `addr`.
    ///
    /// `addr` is any string accepted by [`tokio::net::TcpListener::bind`],
    /// e.g. `"0.0.0.0:5379"` or `"127.0.0.1:5379"`.
    pub async fn run_at(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        Listener::bind(addr).await?.run().await
    }
}
