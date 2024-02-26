use std::net::SocketAddr;
use std::str::FromStr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::task::JoinSet;

use crate::controller::get_context;

const BUFFER_SIZE: usize = 8192;

pub async fn run() {

    let listener = TcpListener::bind("0.0.0.0:9999").await.unwrap();
    loop {
        let client = listener.accept().await;
        if let Ok((conn, addr)) = client {
            tokio::spawn(async move {
                handle_request(conn, addr).await;
            });
        }
    }

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

    let ctx = get_context();
    let read_handle = ctx.nodes.read().await;

    let n_nodes = read_handle.len();
    let target_node = (|| {
        let index = rand::random::<usize>() / n_nodes + 1;
        let target_ip = read_handle.get(index)?;
        Some(format!("{}:9999", target_ip))
    })().unwrap_or("localhost:30800".to_string());
    
    println!("Target node: {target_node}");
    let sock = TcpSocket::new_v4()
        .unwrap()
        .connect(SocketAddr::from_str(&target_node).unwrap())
        .await
        .unwrap();

    proxy(client_conn, sock).await;
    println!("Conexión terminada.");
}
