use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task;

use crate::cmd::RadishCommand;
use crate::response::Response;
use crate::store::Store;

mod connection_guard;
use connection_guard::*;
pub struct Server {}

impl Server {
    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        let host = "127.0.0.1";
        let port = 7379;
        println!("Starting server on {}:{}", host, port);
        let listener = TcpListener::bind(format!("{}:{}", host, port)).await?;
        let store = Rc::new(Store::new());
        let local = task::LocalSet::new();
        let active_connections = Rc::new(RefCell::new(0usize));

        local
            .run_until(async move {
                loop {
                    // Check limit before fully processing a connection
                    if *active_connections.borrow() >= 10_000 {
                        if let Ok((mut stream, _)) = listener.accept().await {
                            let _ = stream.write_all(b"-ERR Server full\r\n").await;
                        }
                        continue;
                    }

                    let (mut stream, addr) = match listener.accept().await {
                        Ok(res) => res,
                        Err(e) => {
                            eprintln!("accept error: {}", e);
                            continue;
                        }
                    };
                    
                    // Disable Nagle's algorithm to fix the artificial 1ms latency delay
                    let _ = stream.set_nodelay(true);
                    
                    println!("accepted new connection from {}", addr);

                    let store_clone = Rc::clone(&store);
                    let guard = ConnectionGuard::new(Rc::clone(&active_connections));

                    task::spawn_local(async move {
                        let _guard = guard; // ensures the connection is tracked until this task drops

                        // Use a single pre-allocated buffer for reading to avoid extend_from_slice and drain overhead
                        let mut buffer = vec![0u8; 8192];
                        let mut head = 0;
                        let mut tail = 0;
                        
                        // A temporary write buffer, reused across requests
                        let mut write_buf = Vec::with_capacity(8192);

                        loop {
                            // If we need more space, shift existing data to the front
                            if tail == buffer.len() {
                                if head > 0 {
                                    buffer.copy_within(head..tail, 0);
                                    tail -= head;
                                    head = 0;
                                } else {
                                    // Buffer is full of unparsed data (huge command), so we must grow it
                                    let new_len = buffer.len() * 2;
                                    if new_len > 1024 * 1024 {
                                        // Max buffer size of 1MB exceeded
                                        let _ = stream.write_all(b"-ERR Maximum buffer size exceeded\r\n").await;
                                        break;
                                    }
                                    buffer.resize(new_len, 0);
                                }
                            }

                            // 1. Read raw bytes directly into the available space with a 30s timeout
                            let read_result = tokio::time::timeout(
                                Duration::from_secs(30),
                                stream.read(&mut buffer[tail..])
                            ).await;

                            let n = match read_result {
                                Ok(Ok(n)) if n == 0 => break,
                                Ok(Ok(n)) => n,
                                Ok(Err(_)) => break,
                                Err(_) => {
                                    // Timeout occurred
                                    let _ = stream.write_all(b"-ERR Connection timed out\r\n").await;
                                    break;
                                }
                            };
                            tail += n;

                            // 2. Parse continuously until we run out of complete commands
                            while head < tail {
                                match RadishCommand::try_parse(&buffer[head..tail]) {
                                    Ok(Some((cmd, bytes_consumed))) => {
                                        head += bytes_consumed;
                                        Response::eval(cmd, &store_clone, &mut write_buf);
                                    }
                                    Ok(None) => {
                                        // Incomplete command, wait for next network read
                                        break; 
                                    }
                                    Err(e) => {
                                        let err_msg = format!("-ERR {}\r\n", e);
                                        write_buf.extend_from_slice(err_msg.as_bytes());
                                        break; // Will drop connection below
                                    }
                                }
                            }

                            // 3. Batch write all responses at once
                            if !write_buf.is_empty() {
                                if stream.write_all(&write_buf).await.is_err() {
                                    return;
                                }
                                write_buf.clear();
                            }

                            // 4. If we consumed everything, instantly reset pointers to 0 
                            // to maximize read space without any memory shifting.
                            if head == tail {
                                head = 0;
                                tail = 0;
                            }
                        }
                    });
                }
            })
            .await;
        Ok(())
    }
}