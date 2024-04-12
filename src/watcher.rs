use std::collections::BTreeMap;
use std::sync::Arc;

use futures::TryStreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{ObjectMeta, PartialObjectMetaExt, Patch, PatchParams};
use kube::runtime::{metadata_watcher, watcher, WatchStreamExt};
use kube::{Api, Client};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use crate::Message;


pub struct AnnotationsWatcher {
    pod_name: String,
    api: Api<Pod>,
    last_annotations: Arc<Mutex<BTreeMap<String, String>>>,
    task_handle: Option<JoinHandle<()>>
}

impl Drop for AnnotationsWatcher {
    fn drop(&mut self) {
        if let Some(handle) = &self.task_handle {
            handle.abort();
        }
    }
}

impl AnnotationsWatcher {

    pub fn new(
        client: Client,
        pod_name: String,
        namespace: String,
        annot_prefix: String,
        sender: mpsc::Sender<Message>
    ) -> Self 
    {
        let api: Api<Pod> = Api::namespaced(client, &namespace);
        let mut watcher = Self {
            pod_name,
            api,
            last_annotations: Arc::new(Mutex::new(BTreeMap::default())),
            task_handle: None
        };

        watcher.run(sender, annot_prefix);
        watcher
    }

    fn run(&mut self, sender: mpsc::Sender<Message>, annot_prefix: String) {

        // Watcher task
        let api = self.api.clone();
        let pod_name = self.pod_name.clone();
        let last_annotations = self.last_annotations.clone();
        let handle = tokio::spawn(async move {
        
            let annot = format!("{annot_prefix}/endpoints");
            let watch_config = watcher::Config::default()
                .fields(&format!("metadata.name={pod_name}"));

            let last_annotations = &last_annotations;
            let annot_name = &annot;
            let sender = &sender;
            metadata_watcher(api, watch_config).touched_objects()
                .into_stream()
                .try_for_each(|pod| async move {
                    
                    if let Some(annots) = pod.metadata.annotations {     
                        
                        let mut previous_annots = last_annotations.lock().await;
                        if let Some(endpoints) = annots.get(annot_name) {
                            
                            // If endpoints have changed
                            if !previous_annots.get(annot_name).is_some_and(|prev_endps| *prev_endps == *endpoints) {

                                let new_endpoints = if endpoints.len() == 0 {
                                    Vec::new()
                                } else {
                                    endpoints.split(",").map(|s| s.to_string()).collect()
                                };
                                
                                log::debug!("Received new endpoints: {:?}", endpoints);
                                // Channel should never be closed, so unwrap won't panic.
                                sender.send(Message::EndpointsChanged(new_endpoints)).await.unwrap();
                            }         
                        }
                        *previous_annots = annots;
                    }
                    
                    Ok(())
                })
                .await;
                
                // El watcher no debería terminar nunca.
                log::error!("Pod watcher exited.");      
        });

        self.task_handle = Some(handle);
    }

    pub async fn add_annot<T>(&self, annots: Vec<(String, T)>)
        where T: ToString
    {
        let mut annotations = self.last_annotations.lock().await;
        
        for annot in annots {
            annotations.insert(annot.0, annot.1.to_string());
        }
        
        let api = self.api.clone();
        let pod_name = self.pod_name.clone();
        // Clone the annotations to send them and drop the mutex.
        let annotations = (*annotations).clone();
        tokio::spawn(async move {

            let meta = ObjectMeta {
                annotations: Some(annotations),
                ..Default::default()
            }.into_request_partial::<Pod>();

            let err = api.patch_metadata(
                &pod_name,
                &PatchParams::apply("tservice-controller"),
                &Patch::Apply(meta)
            )
            .await;

            if let Err(e) = err { log::error!("Error applying patch: {e}"); }
        });

    }
}
