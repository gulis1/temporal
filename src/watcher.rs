use std::collections::BTreeMap;
use std::sync::Arc;

use futures::TryStreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{ObjectMeta, PartialObjectMetaExt, Patch, PatchParams};
use kube::runtime::{metadata_watcher, watcher, WatchStreamExt};
use kube::{Api, Client};
use log::{debug, error};
use tokio::sync::Mutex;
use super::server::ENDPOINTS;
use crate::ANNOT_PREFIX;


pub struct AnnotationsWatcher {
    pod_name: String,
    api: Api<Pod>,
    last_annotations: Arc<Mutex<BTreeMap<String, String>>>,
}

impl AnnotationsWatcher {

    pub fn new(client: Client, pod_name: String, namespace: String) -> Self {

        let api: Api<Pod> = Api::namespaced(client, &namespace);
        let watcher = Self {
            pod_name,
            api,
            last_annotations: Arc::new(Mutex::new(BTreeMap::default())),
        };

        watcher.start();
        watcher
    }

    fn start(&self) {

        // Watcher task
        let api = self.api.clone();
        let pod_name = self.pod_name.clone();
        let last_annotations = self.last_annotations.clone();
        tokio::spawn(async move {
        
            let annot = format!("{ANNOT_PREFIX}/endpoints");
            let watch_config = watcher::Config::default()
                .fields(&format!("metadata.name={pod_name}"));

            let last_annotations = &last_annotations;
            let annot_name = &annot;
            let result = metadata_watcher(api, watch_config).touched_objects()
                .into_stream()
                .try_for_each(|pod| async move {
                    
                    if let Some(annots) = pod.metadata.annotations {     
                        
                        let mut previous_annots = last_annotations.lock().await;
                        if let Some(endpoints) = annots.get(annot_name) {
                            
                            // If endpints have changed
                            if !previous_annots.get(annot_name).is_some_and(|prev_endps| *prev_endps == *endpoints) {
                                 let mut lock = ENDPOINTS.get().unwrap().write().await;
                                 lock.clear();
                                 if endpoints.len() > 0 {
                                     lock.extend(endpoints.split(",").map(|s| s.to_string()));
                                 }
                                 debug!("New endpoints: {:?}", *lock);
                            }         
                        }
                        *previous_annots = annots;
                    }
                    
                    Ok(())
                })
                .await;
                
                // El watcher no debería terminar nunca.
                error!("Pod watcher exited.");      
            result
        });

    }

    pub async fn add_annot<T>(&self, annot: (String, T))
        where T: ToString
    {
        
        let mut annotations = self.last_annotations.lock().await;
        annotations.insert(annot.0, annot.1.to_string());
        
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

            if let Err(e) = err { error!("Error applying patch: {e}"); }
        });

    }
}
