use log::info;
use tokio::sync::RwLock;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::task::JoinSet;
use anyhow::Result;

const BUFFER_SIZE: usize = 8192;

type Endpoints = Arc<RwLock<Vec<String>>>;

pub struct ProxyServer {
    endpoints: Endpoints,
}

impl ProxyServer {
    
    pub async fn new(listen_address: &str) -> Result<Self> {
        
        let server = Self {
            endpoints: Endpoints::default(),
        };
        
        server.run(listen_address).await?;
        Ok(server)
    }

    pub fn update_endpoints(&self, new_endpoints: Vec<String>) {
    
        let endpoints = Arc::clone(&self.endpoints.clone());
        tokio::spawn(async move {
            let mut write_handle = endpoints.write().await;
            *write_handle = new_endpoints;
        });
    }

    async fn run(&self, listen_address: &str) -> Result<()> {
        
        let listener = TcpListener::bind(listen_address).await?;
        let endpoints = Arc::clone(&self.endpoints); 
        tokio::spawn(async move {
            loop {
                let client = listener.accept().await;
                if let Ok((conn, addr)) = client {
                    let endpts = endpoints.clone();
                    tokio::spawn(async move {
                        Self::handle_request(conn, addr, endpts).await;
                    });
                }
            }
        });

        Ok(())
    }

    async fn handle_request(client_conn: TcpStream, _: SocketAddr, endpoints: Endpoints) {

        let read_handle = endpoints.read().await;

        let n_nodes = read_handle.len();
        let target_node = (|| {
            let index = rand::random::<usize>().checked_rem(n_nodes + 1)?;
            let target_ip = read_handle.get(index)?;
            Some(format!("{}:9999", target_ip))
        })().unwrap_or("127.0.0.1:8000".to_string());
       
       info!("Target node: {target_node}");
       let sock = TcpSocket::new_v4()
           .unwrap()
           .connect(SocketAddr::from_str(&target_node).unwrap())
           .await
           .unwrap();

       Self::proxy(client_conn, sock).await;
       info!("Conexión terminada.");
    }

    async fn proxy(client: TcpStream, triton: TcpStream) {

        let (mut client_recv, mut client_sender) = client.into_split();
        let (mut triton_recv, mut triton_sender) = triton.into_split();

        let mut set: JoinSet<Result<(), std::io::Error>> = JoinSet::new();
        set.spawn(async move {
            
            let mut buff = vec![0; BUFFER_SIZE];
            loop {
                let n = client_recv.read(&mut buff).await?;
                if n == 0 { break; }
                triton_sender.write_all(&buff[..n]).await?;
            }

            Ok(())
        });

        set.spawn(async move {

            let mut buff = vec![0; BUFFER_SIZE];
            loop {
                let n = triton_recv.read(&mut buff).await?;
                if n == 0 { break; }
                client_sender.write_all(&buff[..n]).await?;
            }

            Ok(())
        });

        set.join_next().await;
        set.abort_all();
    }
}




