mod watcher;
mod server;
mod hardware;

use std::env;
use hardware::get_hardware_info;
use kube::{Client, Config};
use log::{error, info};
use anyhow::{anyhow, Context, Result};
use watcher::AnnotationsWatcher;

pub const ANNOT_PREFIX: &str = "tritonservices.prueba.ucm.es";

#[tokio::main]
async fn main() -> Result<()> {

    if cfg!(debug_assertions) {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Debug)
        .init();
    }
    else { env_logger::init(); }

    let cluster_config = Config::incluster();
    let client = match cluster_config {
        Ok(config) => {
            info!("Running triton_proxy inside cluster.");
            Client::try_from(config)
        }
        Err(_) => {
            info!("Running triton_proxy outside cluster.");
            Client::try_default().await
        }
    };

    let (pod_namespace, pod_name) = get_env_vars()?;
    match client {
        Ok(client) => {
            let hw_info = get_hardware_info(); 
            info!("Detected hardware: {:?}", hw_info);
            let watcher = AnnotationsWatcher::new(client, pod_name, pod_namespace);
            watcher.add_annot((format!("{ANNOT_PREFIX}/hw_info"), hw_info)).await;
            server::start_server();
            tokio::signal::ctrl_c().await.unwrap();
            Ok(())
        },
        Err(e) => {
            error!("Failed to start Kubernetes api client: {e}");
            Err(anyhow!("Failed to start Kubernetes client"))
        }
    }
}

fn get_env_vars() -> Result<(String, String)> {
    
    let pod_namespace = env::var("POD_NAMESPACE").context("Missing POD_NAMESPACE env variable.")?;
    let pod_name = env::var("POD_NAME").context("Missing POD_NAME env variable.")?;
    info!("Retrieved env variables: POD_NAMESPACE={pod_namespace}, POD_NAME={pod_name}");
    Ok((pod_namespace, pod_name))
}
