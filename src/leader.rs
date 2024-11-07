// src/leader.rs
use crate::network::{receive_message, send_message};
use crate::types::PaxosMessage;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{timeout, Duration};
use uuid::Uuid;

pub async fn leader_main(leader_addr: &str) {
    let socket = Arc::new(UdpSocket::bind(leader_addr).await.unwrap());
    println!("Leader running at {}", leader_addr);

    let followers = Arc::new(Mutex::new(Vec::new()));
    let acks = Arc::new(Mutex::new(0));

    // Channel to send acknowledgment notifications from the receiver task to the main task
    let (ack_tx, mut ack_rx) = mpsc::channel(32);

    // Background task for receiving acknowledgments
    {
        let socket = Arc::clone(&socket);
        let acks = Arc::clone(&acks);
        let ack_tx = ack_tx.clone();
        tokio::spawn(async move {
            loop {
                if let Ok((message, src_addr)) = receive_message(&socket).await {
                    match message {
                        PaxosMessage::FollowerAck { request_id } => {
                            let mut acks_guard = acks.lock().await;
                            *acks_guard += 1;
                            println!(
                                "Leader received acknowledgment from follower at {}",
                                src_addr
                            );
                            let _ = ack_tx.send(request_id).await; // Notify main task of acknowledgment
                        }
                        _ => {}
                    }
                }
            }
        });
    }

    loop {
        let (message, src_addr) = receive_message(&socket).await.unwrap();

        match message {
            PaxosMessage::RegisterFollower { follower_addr } => {
                // Use named field syntax
                let mut followers_guard = followers.lock().await;
                followers_guard.push(follower_addr.clone());
                println!("Follower registered: {}", follower_addr);
            }
            PaxosMessage::ClientRequest { payload, .. } => {
                let original_message = String::from_utf8_lossy(&payload).to_string();
                println!("Leader received request from client: {}", original_message);

                // Generate a unique request ID using UUID
                let request_id = Uuid::new_v4();
                println!("Generated UUID for request: {}", request_id);

                let follower_list = {
                    let followers_guard = followers.lock().await;
                    followers_guard.clone()
                };

                if follower_list.is_empty() {
                    println!("No followers registered. Cannot proceed.");
                    continue;
                }

                // Reset acknowledgments
                {
                    let mut acks_guard = acks.lock().await;
                    *acks_guard = 0;
                }
                let majority = follower_list.len() / 2 + 1;

                // Send requests concurrently to all followers
                for follower_addr in &follower_list {
                    let socket_clone = Arc::clone(&socket);
                    let payload_clone = payload.clone();
                    let follower_addr_clone = follower_addr.clone();
                    let request_id_clone = request_id;

                    tokio::spawn(async move {
                        let request_message = PaxosMessage::ClientRequest {
                            request_id: request_id_clone,
                            payload: payload_clone,
                        };

                        if let Err(e) =
                            send_message(&socket_clone, request_message, &follower_addr_clone).await
                        {
                            println!(
                                "Failed to send request to follower {}: {}",
                                follower_addr_clone, e
                            );
                        } else {
                            println!("Leader sent request to follower at {}", follower_addr_clone);
                        }
                    });
                }

                // Wait for majority of acknowledgments using the channel, with timeout
                let mut received_acks = 0;
                while let Ok(Some(ack_request_id)) =
                    timeout(Duration::from_secs(2), ack_rx.recv()).await
                {
                    if ack_request_id == request_id {
                        received_acks += 1;
                        if received_acks >= majority {
                            println!(
                                "Leader received majority acknowledgment for request ID {}",
                                request_id
                            );
                            break;
                        }
                    }
                }

                // Check if majority was not reached (in case of timeout)
                if received_acks < majority {
                    println!("Not enough acknowledgments for request ID {}", request_id);
                }
            }
            _ => {}
        }
    }
}
