use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        let stream = listener.accept().await;
        match stream {
            Ok((mut _stream, addr)) => {
                println!("accepted new connection from {}", addr);

                tokio::spawn(async move {
                    let mut buf = [0; 1024];
                    loop {
                        // will it get interupted by system signal?
                        let read_count = _stream.read(&mut buf).await.unwrap();
                        if read_count == 0 {
                            break;
                        }
                        _stream.write_all(&buf).await.unwrap();
                    }
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
