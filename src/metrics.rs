use std::{env, str::FromStr, time::Duration};
use prometheus_http_query::Client;
use tokio::{sync::mpsc, task::JoinHandle, time::interval};
use anyhow::Result;
use crate::Message;

pub struct PrometheusClient {
    task_handle: Option<JoinHandle<()>>
}

impl Drop for PrometheusClient {
    fn drop(&mut self) {
        if let Some(handle) = &self.task_handle {
            handle.abort();
        }
    }
}

impl PrometheusClient {

    pub fn new(target: &str, annot_prefix: String, sender: mpsc::Sender<Message>) -> Result<Self> {

        let mut client = Self {
            task_handle: None
        };

        client.run(target, annot_prefix, sender);
        Ok(client)
    }

    fn run(&mut self, target: &str, annot_prefix: String, sender: mpsc::Sender<Message>) -> Result<()> {
    
        let client = Client::from_str("http://ocejon.dacya.ucm.es:9090")?;
        let interval_secs: u64 = env::var("METRICS_QUERY_INTERVAL_SECS")
            .map(|var| var.parse())
            .unwrap_or(Ok(5))?;

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));
            loop {

                interval.tick().await;
                let response = client.query("nv_inference_count").get().await;
                println!("{:?}", response);
            }
        });
        
        self.task_handle = Some(handle);
        Ok(())
    }
}
