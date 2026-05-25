use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task;

use crate::cmd::RadishCommand;
use crate::response::Response;
use crate::store::Store;

pub(crate) struct Server {}

impl Server {
    pub(crate) async fn run() -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:7379").await?;
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

                    let store_clone = Store::clone_shared(&store);

                    task::spawn_local(async move {
                        let mut buf = [0; 512];

                        loop {
                            match stream.read(&mut buf).await {
                                Ok(0) => {
                                    println!("client disconnected");
                                    break;
                                }

                                Ok(read_count) => {
                                    let cmd = RadishCommand::from_bytes(&buf[..read_count]);
                                    match cmd {
                                        Some(cmd) => {
                                            let response = Response::eval(&cmd, &store_clone);
                                            if let Err(err) = stream.write_all(&response.data).await
                                            {
                                                eprintln!("write error: {}", err);
                                                break;
                                            }
                                        }
                                        None => {
                                            let error_response = b"-ERR invalid command\r\n";
                                            if let Err(err) = stream.write_all(error_response).await
                                            {
                                                eprintln!("write error: {}", err);
                                                break;
                                            }
                                        }
                                    }
                                }

                                Err(err) => {
                                    eprintln!("read error: {}", err);
                                    break;
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
