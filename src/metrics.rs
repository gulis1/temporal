use std::str::FromStr;

use prometheus_http_query::Client;
// pub struct NodeInfo {
//     nodes: Arc<Mutex<<HashMap<Sring, String>>
// }


pub async fn get_metrics(address: &str) {
    
    // let client = Client::from_str(format!("http://{address}:9090"));
    
    let url = format!("http://{address}:8002/metrics");
    let response = reqwest::get(&url).await;
    if let Ok(metrics) = response {
        println!("{:?}", metrics.text().await)
    }
}