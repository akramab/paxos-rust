use crate::types::PaxosMessage;
use bincode;
use tokio::io;
use tokio::net::UdpSocket;

pub async fn send_message(socket: &UdpSocket, message: PaxosMessage, addr: &str) -> io::Result<()> {
    let serialized = bincode::serialize(&message).unwrap();
    socket.send_to(&serialized, addr).await?;
    Ok(())
}

pub async fn receive_message(socket: &UdpSocket) -> io::Result<(PaxosMessage, String)> {
    let mut buffer = vec![0; 1024];
    let (size, src) = socket.recv_from(&mut buffer).await?;
    let message: PaxosMessage = bincode::deserialize(&buffer[..size]).unwrap();
    Ok((message, src.to_string()))
}
