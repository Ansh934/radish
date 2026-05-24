mod cmd;
mod resp;
mod response;
mod server;
mod store;

use server::Server;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    println!("Logs from your program will appear here!");
    if let Err(e) = Server::run().await {
        eprintln!("Server error: {}", e);
    }
}
