#[macro_use]
extern crate lazy_static;

mod controller;
mod server;

#[tokio::main]
async fn main() {

    env_logger::init();
    tokio::spawn(async { controller::run().await; });
    tokio::spawn(async { server::run().await; });
    tokio::signal::ctrl_c().await.unwrap();
}
