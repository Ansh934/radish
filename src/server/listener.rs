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
            store: Store::new(),
            active_connections: Rc::new(RefCell::new(0)),
        })
    }

    /// Runs the accept loop, blocking until the process exits.
    pub(crate) async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let local = task::LocalSet::new();
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

                    let guard = ConnectionGuard::new(Rc::clone(&self.active_connections));
                    let conn = Connection::new(stream, Rc::clone(&self.store), guard);
                    task::spawn_local(conn.run());
                }
            })
            .await;
        Ok(())
    }
}
