use crate::network::{receive_message, send_message};
use crate::types::PaxosMessage;
use chrono::Utc;
use redis::{AsyncCommands, RedisResult};
use tokio::net::UdpSocket;
use tokio::time::{interval, Duration, Instant};
use uuid::Uuid;

pub async fn follower_main(node_id: Uuid, follower_addr: &str, leader_addr: &str) {
    let socket = UdpSocket::bind(follower_addr).await.unwrap();
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_async_connection().await.unwrap();

    println!("Follower {} listening on {}", node_id, follower_addr);

    let mut check_interval = interval(Duration::from_secs(3));
    let mut last_heartbeat = Instant::now();

    loop {
        tokio::select! {
            _ = check_interval.tick() => {
                let heartbeat_data: RedisResult<String> = con.get("current_leader").await;

                match heartbeat_data {
                    Ok(data) => {
                        let parts: Vec<&str> = data.split(':').collect();
                        if parts.len() == 2 {
                            let leader_id = parts[0];
                            let timestamp: i64 = parts[1].parse().unwrap_or(0);
                            let current_timestamp = Utc::now().timestamp();

                            if current_timestamp - timestamp > 4 {
                                println!("Follower {}: Detected stale heartbeat, initiating leader election...", node_id);
                                // Start leader election process
                                initiate_leader_election(node_id, &mut con).await;
                            } else {
                                println!("Follower {}: Leader is active with ID: {}", node_id, leader_id);
                                last_heartbeat = Instant::now(); // Update last heartbeat received time
                            }
                        }
                    },
                    Err(e) => eprintln!("Error reading leader heartbeat from Redis: {}", e),
                }
            }

            // Process leader-to-follower communication
            Ok((message, src_addr)) = receive_message(&socket) => {
                match message {
                    PaxosMessage::ClientRequest { request_id, payload } => {
                        println!("Follower {} received request from leader: {:?}", node_id, payload);
                        // Send acknowledgment back to the leader
                        send_message(&socket, PaxosMessage::FollowerAck { request_id }, leader_addr).await.unwrap();
                        println!("Follower {} acknowledged request ID: {}", node_id, request_id);
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn initiate_leader_election(node_id: Uuid, con: &mut redis::aio::Connection) {
    // Code to initiate leader election
    println!(
        "Follower {} is attempting to become the new leader...",
        node_id
    );
    let _: () = con
        .set(
            "current_leader",
            format!("{}:{}", node_id, Utc::now().timestamp()),
        )
        .await
        .unwrap();
    println!(
        "Follower {} has declared itself as the new leader.",
        node_id
    );
}
