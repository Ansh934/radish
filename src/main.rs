use radish::Server;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(e) = Server::run().await {
        eprintln!("Server error: {}", e);
    }
}

