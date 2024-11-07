// src/follower.rs
use crate::network::{receive_message, send_message};
use crate::types::PaxosMessage;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

pub async fn follower_main(follower_addr: &str, leader_addr: &str) {
    let socket = UdpSocket::bind(follower_addr).await.unwrap();
    println!("Follower running at {}", follower_addr);

    // Register with the leader
    let registration_message = PaxosMessage::RegisterFollower {
        follower_addr: follower_addr.to_string(),
    };
    send_message(&socket, registration_message, leader_addr)
        .await
        .expect("Failed to register with leader");

    loop {
        let (message, src_addr) = receive_message(&socket).await.unwrap();

        match message {
            PaxosMessage::ClientRequest {
                request_id,
                payload,
            } => {
                let original_message = String::from_utf8_lossy(&payload).to_string();
                println!(
                    "Follower received request from leader: {}",
                    original_message
                );

                // Acknowledge the leader's request
                let ack_message = PaxosMessage::FollowerAck { request_id };
                send_message(&socket, ack_message, leader_addr)
                    .await
                    .expect("Failed to send acknowledgment to leader");
                println!("Follower acknowledged request ID: {}", request_id);
            }
            _ => {}
        }
    }
}
