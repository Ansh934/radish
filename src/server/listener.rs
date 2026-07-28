use std::cell::RefCell;
use std::rc::Rc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::task;

use super::connection::{Connection, ConnectionGuard};
use crate::storage::Store;

const MAX_CONNECTIONS: usize = 10_000;
const SERVER_FULL_RESPONSE: &[u8] = b"-ERR Server full\r\n";

/// A bound TCP listener that owns the accept loop.
///
/// `Listener` holds the socket, the shared store, and the connection counter.
/// It is created via [`Listener::bind`] and consumed by [`Listener::run`].
pub(crate) struct Listener {
    tcp: TcpListener,
    store: crate::storage::SharedStore,
    active_connections: Rc<RefCell<usize>>,
}

impl Listener {
    /// Binds a TCP socket to `addr` and returns a ready-to-run `Listener`.
    pub(crate) async fn bind(addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        println!("Starting server on {}", addr);
        Ok(Self {
            tcp: TcpListener::bind(addr).await?,
            store: Store::with_capacity(10), 
            active_connections: Rc::new(RefCell::new(0)),
        })
    }

    /// Runs the accept loop, blocking until the process exits.
    pub(crate) async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let local = task::LocalSet::new();
        let store_for_cleanup = Rc::clone(&self.store);
        local.spawn_local(async move {
            // Time Period for cleanup of expired entries in the store
            let duration_in_secs: u64 = 10;
            let max_expired_entries_allowed: f64 = 0.25;
            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs(duration_in_secs));
            loop {
                interval.tick().await;
                loop {
                    let frac = store_for_cleanup.borrow_mut().cleanup_expired_entries(20);
                    if frac < max_expired_entries_allowed {
                        break;
                    }
                }
                println!(
                    "Ran cleanup. Store size: {}",
                    store_for_cleanup.borrow().len()
                );
            }
        });

        local
            .run_until(async move {
                loop {
                    // Reject immediately when at capacity.
                    if *self.active_connections.borrow() >= MAX_CONNECTIONS {
                        if let Ok((mut stream, _)) = self.tcp.accept().await {
                            let _ = stream.write_all(SERVER_FULL_RESPONSE).await;
                        }
                        continue;
                    }

                    // Accept a new connection.
                    let (stream, _) = match self.tcp.accept().await {
                        Ok(res) => res,
                        Err(e) => {
                            eprintln!("accept error: {}", e);
                            continue;
                        }
                    };

                    // Disable Nagle's algorithm — eliminates the artificial 1 ms
                    // latency that small-write batching would otherwise introduce.
                    let _ = stream.set_nodelay(true);

                    // Spawn a new task to handle the connection.
                    let guard = ConnectionGuard::new(Rc::clone(&self.active_connections));
                    let conn = Connection::new(stream, Rc::clone(&self.store), guard);
                    task::spawn_local(conn.run());
                }
            })
            .await;
        Ok(())
    }
}
