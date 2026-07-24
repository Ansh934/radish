use super::listener::Listener;

const DEFAULT_ADDR: &str = "127.0.0.1:6379";

/// Top-level server handle.
///
/// # Examples
/// ```no_run
/// // Start on the default address (127.0.0.1:6379)
/// Server::run().await?;
///
/// // Start on a custom address
/// Server::run_at("0.0.0.0:7379").await?;
/// ```
// `server` inside the `server` module is intentional — it mirrors the
// real-world convention of a `server/server.rs` pairing.
#[allow(clippy::module_inception)]
pub struct Server;

impl Server {
    /// Starts the server on the default address (`127.0.0.1:6379`).
    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        Self::run_at(DEFAULT_ADDR).await
    }

    /// Starts the server bound to `addr`.
    ///
    /// `addr` is any string accepted by [`tokio::net::TcpListener::bind`],
    /// e.g. `"0.0.0.0:6379"` or `"127.0.0.1:6379"`.
    pub async fn run_at(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        Listener::bind(addr).await?.run().await
    }
}
