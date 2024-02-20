use std::{sync::Arc, time::Duration};
use k8s_openapi::api::core::v1::Pod;
use kube::runtime::{controller::Action, watcher::Config, Controller};
use kube::{ Api, Client};
use futures::StreamExt;
use thiserror::Error;

use crate::metrics::get_metrics;

static mut COUNTER: i64 = 0;

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
#[derive(Clone)]
struct Context {}

fn error_policy(_doc: Arc<Pod>, _error: &Error, _ctx: Arc<Context>) -> Action {
    println!("Error");
    Action::requeue(Duration::from_secs(5 * 60))
}

async fn reconcile(pod: Arc<Pod>, _ctx: Arc<Context>) -> Result<Action> {

    let ip = pod.status.as_ref().and_then(|status| status.pod_ip.clone());
    println!("{:?}", pod.status.as_ref().unwrap());
    if let Some(ip) = ip {
        unsafe {
            println!("{COUNTER}: {}", ip);
            COUNTER += 1;
        }
    }
    else { println!("Error"); }
    

    // for endpoint in endpoints {
    //     get_metrics(&endpoint).await;
    // }
    Ok(Action::requeue(Duration::from_secs(5 * 60))) 
}

pub async fn run() {

    let client = Client::try_default().await.expect("failed to create kube Client");
    let endpoints = Api::<Pod>::namespaced(client, "kube-triton");
    let context = Arc::new(Context{});

    Controller::new(endpoints, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(reconcile, error_policy, context)
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}