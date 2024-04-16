mod watcher;
mod server;
mod hardware;
mod metrics;

use std::env;
use hardware::get_hardware_info;
use kube::{Client, Config};
use log::{error, info};
use anyhow::{anyhow, Context, Result};
use metrics::PrometheusClient;
use watcher::AnnotationsWatcher;

use crate::server::ProxyServer;

pub const ANNOT_PREFIX: &str = "tritonservices.prueba.ucm.es";

#[derive(Debug, Clone)]
pub enum Message {
    EndpointsChanged(Vec<String>),
    AnnotationUpdate(Vec<(String, String)>)
}

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
        Ok(client) => main_task(client, pod_namespace, pod_name).await,
        Err(e) => {
            error!("Failed to start Kubernetes api client: {e}");
            Err(anyhow!("Failed to start Kubernetes client"))
        }
    }
}

async fn main_task(
    client: Client, 
    pod_namespace: String, 
    pod_name: String
) -> Result<()>
{
    let hw_info = get_hardware_info(); 
    info!("Detected hardware: {:?}", hw_info);

    let (sender, mut receiver) = tokio::sync::mpsc::channel::<Message>(32);
    let proxy_server = ProxyServer::new("0.0.0.0:9999").await?;
    let watcher = AnnotationsWatcher::new(
        client,
        pod_name,
        pod_namespace,
        ANNOT_PREFIX.to_string(),
        sender.clone()
    );

    let _metrics_client = PrometheusClient::new("http://localhost:9090",
        ANNOT_PREFIX.to_string(),
        sender.clone())?;

    // Update the server with the probed hardware.
    watcher.add_annot(vec![(format!("{ANNOT_PREFIX}/hw_info"), hw_info)]).await;
    loop {
        
        let message = receiver.recv().await.context("Message channel closed.")?;
        log::debug!("New message received: {:?}", message);
        match message {
            Message::EndpointsChanged(endpoints) => {
                proxy_server.update_endpoints(endpoints);
            },
            Message::AnnotationUpdate(annots) => {
                watcher.add_annot(annots).await;
            }
        }
    }
}

fn get_env_vars() -> Result<(String, String)> {
    
    let pod_namespace = env::var("POD_NAMESPACE").context("Missing POD_NAMESPACE env variable.")?;
    let pod_name = env::var("POD_NAME").context("Missing POD_NAME env variable.")?;
    info!("Retrieved env variables: POD_NAMESPACE={pod_namespace}, POD_NAME={pod_name}");
    Ok((pod_namespace, pod_name))
}
