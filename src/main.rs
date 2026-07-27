use radish::Server;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(e) = Server::run_at("127.0.0.1:5379").await {
        eprintln!("Server error: {}", e);
    }
}

