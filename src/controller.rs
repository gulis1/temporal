use std::{sync::Arc, time::Duration};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::runtime::watcher;
use kube::runtime::{controller::Action, Controller};
use kube::{Api, Client, Config};
use futures::StreamExt;
use thiserror::Error;
use tokio::sync::RwLock;
use std::env;

lazy_static! {
    static ref CONTEXT: Arc<Context> = Arc::new(Context {
        current_pod_ip: env::var("POD_IP").unwrap(),
        nodes: RwLock::new(Vec::new())
    });
}

#[derive(Error, Debug)]
enum Error {

    // #[error("Kube Error: {0}")]
    // KubeError(#[source] kube::Error),

    // #[error("Finalizer Error: {0}")]
    // // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // // so boxing this error to break cycles
    // FinalizerError(#[source] Box<kube::runtime::finalizer::Error<Error>>)
}
type Result<T, E = Error> = std::result::Result<T, E>;

// Context for our reconciler
pub struct Context {
    current_pod_ip: String,
    pub nodes: RwLock<Vec<String>>
}

fn error_policy(_doc: Arc<EndpointSlice>, _error: &Error, _ctx: Arc<Context>) -> Action {
    println!("Error");
    Action::requeue(Duration::from_secs(5 * 60))
}

async fn reconcile(endpoint: Arc<EndpointSlice>, ctx: Arc<Context>) -> Result<Action> {
    
    let nodes= endpoint.endpoints.iter()
        .filter(|node| node.conditions
            .as_ref()
            .and_then(|conds| conds.ready)
            .is_some_and(|is_ready| is_ready)
        )
        .filter_map(|node| node.addresses.get(0).map(|addr| addr.clone()))
        .filter(|addr| addr != &ctx.current_pod_ip);

    let mut write_handle = ctx.nodes.write().await;
    write_handle.clear();
    write_handle.extend(nodes);

    println!("{:?}", write_handle.as_slice());
    drop(write_handle);
    Ok(Action::requeue(Duration::from_secs(5 * 60))) 
}

pub async fn run() {

    let cluster_config = Config::incluster();
    let client = match cluster_config {
        Ok(config) => {
            println!("Running triton_proxy inside cluster.");
            Client::try_from(config)
        }
        Err(_) => {
            println!("Running triton_proxy outside cluster.");
            Client::try_default().await
        }
    }.expect("Failed to create Kubernetes client.");
    

    let endpoints = Api::<EndpointSlice>::namespaced(client, "kube-triton");

    let context = CONTEXT.clone();
    Controller::new(endpoints, watcher::Config::default().any_semantic())
        .shutdown_on_signal( )
        .run(reconcile, error_policy, context)
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

pub fn get_context() -> Arc<Context> {
    CONTEXT.clone()
}
