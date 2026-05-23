mod resp;
mod cmd;
mod server;

use server::Server;

#[tokio::main]
async fn main() {
    println!("Logs from your program will appear here!");
    Server::start().await;
}
