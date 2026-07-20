use std::rc::Rc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task;

use crate::cmd::RadishCommand;
use crate::response::Response;
use crate::store::Store;

pub(crate) struct Server {}

impl Server {
    pub(crate) async fn run() -> Result<(), Box<dyn std::error::Error>> {
        let host = "127.0.0.1";
        let port = 7379;
        println!("Starting server on {}:{}", host, port);
        let listener = TcpListener::bind(format!("{}:{}", host, port)).await?;
        let store = Store::new();
        let local = task::LocalSet::new();

        local
            .run_until(async move {
                loop {
                    let (mut stream, addr) = match listener.accept().await {
                        Ok(res) => res,
                        Err(e) => {
                            eprintln!("accept error: {}", e);
                            continue;
                        }
                    };
                    println!("accepted new connection from {}", addr);

                    let store_clone = Rc::clone(&store);

                    task::spawn_local(async move {
                        loop {
                            let mut buf: Vec<u8> = Vec::new();
                            if let Err(e) = stream.read_to_end(&mut buf).await {
                                eprintln!("read error: {}", e);
                                break;
                            }
                            let cmd = RadishCommand::from_bytes(buf.into());
                            match cmd {
                                Ok(cmd) => {
                                    let response = Response::eval(cmd, &store_clone);
                                    if let Err(err) = stream.write_all(&response.data).await {
                                        eprintln!("write error: {}", err);
                                        break;
                                    }
                                }
                                Err(command_err) => {
                                    eprintln!("decode error: {}", command_err);
                                    let error_response = b"-ERR invalid command\r\n";
                                    if let Err(err) = stream.write_all(error_response).await {
                                        eprintln!("write error: {}", err);
                                        break;
                                    }
                                }
                            }
                        }
                    });
                }
            })
            .await;
        Ok(())
    }
}
