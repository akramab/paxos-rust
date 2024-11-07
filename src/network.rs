use crate::types::PaxosMessage;
use bincode;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use uuid::Uuid;

pub async fn receive_message(
    socket: &UdpSocket,
) -> Result<(PaxosMessage, SocketAddr), Box<dyn std::error::Error + Send + Sync>> {
    let mut buf = vec![0; 1024];
    let (size, src_addr) = socket.recv_from(&mut buf).await?;
    buf.truncate(size);

    match bincode::deserialize::<PaxosMessage>(&buf) {
        Ok(message) => Ok((message, src_addr)),
        Err(_) => {
            let payload = buf.clone();
            let request_id = Uuid::new_v4(); // Generate a UUID instead of u64
            let message = PaxosMessage::ClientRequest {
                request_id,
                payload,
            };
            Ok((message, src_addr))
        }
    }
}

pub async fn send_message(
    socket: &UdpSocket,
    message: PaxosMessage,
    addr: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let serialized = bincode::serialize(&message)?;
    socket.send_to(&serialized, addr).await?;
    Ok(())
}
