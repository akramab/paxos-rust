use crate::network::{receive_message, send_message};
use crate::types::PaxosMessage;
use chrono::Utc;
use redis::{AsyncCommands, RedisResult};
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Receiver;
use tokio::time::{interval, Duration};
use uuid::Uuid; // For timestamping

pub async fn leader_main(node_id: Uuid, leader_addr: &str, mut ack_rx: Receiver<Uuid>) {
    let socket = UdpSocket::bind(leader_addr).await.unwrap();
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_async_connection().await.unwrap();

    println!("Leader with ID {} running at {}", node_id, leader_addr);

    let mut heartbeat_interval = interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            // Send heartbeat with timestamp to Redis
            _ = heartbeat_interval.tick() => {
                let leader_id = node_id.to_string();
                let timestamp = Utc::now().timestamp();
                let heartbeat_data = format!("{}:{}", leader_id, timestamp);

                let result: RedisResult<()> = con.set("current_leader", heartbeat_data).await;
                match result {
                    Ok(_) => println!("Leader heartbeat sent to Redis with ID: {} and timestamp: {}", leader_id, timestamp),
                    Err(e) => eprintln!("Failed to send heartbeat to Redis: {}", e),
                }
            }

            // Process incoming client requests
            Ok((message, src_addr)) = receive_message(&socket) => {
                match message {
                    PaxosMessage::ClientRequest { request_id, payload } => {
                        println!("Leader received request from client: {:?}", payload);

                        // Send the request to all followers
                        let follower_list = vec!["127.0.0.1:8081", "127.0.0.1:8082", "127.0.0.1:8083"];
                        for follower_addr in &follower_list {
                            send_message(&socket, PaxosMessage::ClientRequest { request_id, payload: payload.clone() }, follower_addr).await.unwrap();
                            println!("Leader sent request to follower at {}", follower_addr);
                        }

                        // Wait for majority of acknowledgments asynchronously
                        let mut acks = 0;
                        let majority = follower_list.len() / 2 + 1;
                        while acks < majority {
                            match ack_rx.recv().await {
                                Some(ack_id) if ack_id == request_id => {
                                    acks += 1;
                                    println!("Leader received acknowledgment for request ID {}", request_id);
                                },
                                _ => break,
                            }
                        }

                        if acks >= majority {
                            println!("Leader received majority acknowledgment for request ID {}", request_id);
                        } else {
                            println!("Leader did not receive majority acknowledgment for request ID {}", request_id);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
