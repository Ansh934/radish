use radish::Server;
#[tokio::main(flavor = "current_thread")]
async fn main() {
    println!("Logs from your program will appear here!");
    if let Err(e) = Server::run().await {
        eprintln!("Server error: {}", e);
    }
}
