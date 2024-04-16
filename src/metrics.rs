use std::{collections::HashMap, env, str::FromStr, sync::Arc, time::Duration};
use prometheus_http_query::{response::PromqlResult, Client};
use tokio::{sync::mpsc, task::{JoinHandle, JoinSet}, time::interval};
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

        client.run(target, annot_prefix, sender)?;
        Ok(client)
    }

    fn run(&mut self, target: &str, annot_prefix: String, sender: mpsc::Sender<Message>) -> Result<()> {
    
        let client = Arc::new(Client::from_str(target)?);
        let interval_secs: u64 = env::var("METRICS_QUERY_INTERVAL_SECS")
            .map(|var| var.parse())
            .unwrap_or(Ok(5))?;

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));
            loop {
                let client = client.clone();
                match Self::query_metrics(client).await {
                    Ok(metrics) => {
                        let annot_name = format!("{annot_prefix}/triton_metrics");
                        let json = serde_json::to_string_pretty(&metrics).unwrap();
                        log::debug!("{annot_name}:\n{json}");
                        sender.send(Message::AnnotationUpdate(vec![(annot_name, json)]))
                            .await
                            .expect("Failed to send message.");
                    },
                    Err(e) => log::error!("Failed to query Prometheus metrics: {e}")
                }
                interval.tick().await;

            }
        });
        
        self.task_handle = Some(handle);
        Ok(())
    }

    async fn query_metrics(client: Arc<Client>) -> Result<HashMap<String, f64>> {
        
        // Query the names of all available metrics.
        let metric_names = client.label_values("__name__")
            .get()
            .await?;

        // Spawn a task to query each metric.
        let n_metrics = metric_names.len();
        let mut joinset = JoinSet::new();
        for metric in metric_names {
            let client = Arc::clone(&client);
            joinset.spawn(async move { client.query(metric).get().await });
        }

        let mut metrics = HashMap::with_capacity(n_metrics);
        while let Some(res) = joinset.join_next().await {
            match res? {
                Ok(result) => {
                    if let Some(metric) = get_metric(result) {
                        metrics.insert(metric.0, metric.1);
                    }
                },
                Err(e) => log::error!("Failed to query metric: {e}"),
            }
        }

        Ok(metrics)
    }
}

fn get_metric(result: PromqlResult) -> Option<(String, f64)> {

    let (mut info, sample) = result.into_inner().0
        .into_vector()
        .map(|mut vec| vec.remove(0))
        .map(|metric| {
            metric.into_inner()
        }).ok()?;

    Some((info.remove("__name__")?, sample.value()))
}
