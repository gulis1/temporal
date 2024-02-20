use tokio::task::JoinSet;

mod controller;
mod metrics;
mod server;

#[tokio::main]
async fn main() {

    let mut set = JoinSet::new();
    // set.spawn(controller::run());
    set.spawn(server::run());

    while let Some(_) = set.join_next().await { }
}
