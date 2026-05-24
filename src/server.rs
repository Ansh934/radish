use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::cmd::RadishCommand;

pub(crate) struct Server {}

impl Server {
    pub(crate) async fn start() -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:7379").await?;
        loop {
            let (mut stream, addr) = listener.accept().await?;
            println!("accepted new connection from {}", addr);
            tokio::spawn(async move {
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
                                    let response = cmd.eval();
                                    if let Err(err) = stream.write_all(&response).await {
                                        eprintln!("write error: {}", err);
                                        break;
                                    }
                                }
                                None => {
                                    let error_response = b"-ERR invalid command\r\n";
                                    if let Err(err) = stream.write_all(error_response).await {
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
    }
}
