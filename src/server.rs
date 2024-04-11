use log::info;
use tokio::sync::RwLock;
use std::net::SocketAddr;
use std::str::FromStr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::task::JoinSet;
use std::sync::OnceLock;

const BUFFER_SIZE: usize = 8192;

pub static ENDPOINTS: OnceLock<RwLock<Vec<String>>> = OnceLock::new(); 

pub fn start_server() {

    ENDPOINTS.set(RwLock::new(Vec::new())).expect("Server has already been started.");
    tokio::spawn(async move {
        let listener = TcpListener::bind("0.0.0.0:9999").await.unwrap();
        loop {
            let client = listener.accept().await;
            if let Ok((conn, addr)) = client {
                tokio::spawn(async move {
                    handle_request(conn, addr).await;
                });
            }
        }
    });
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

async fn handle_request(client_conn: TcpStream, _: SocketAddr) {

   let read_handle = ENDPOINTS.get().unwrap().read().await;

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

   proxy(client_conn, sock).await;
   info!("Conexión terminada.");
}
