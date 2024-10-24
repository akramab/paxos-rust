// src/leader.rs
use crate::network::{receive_message, send_message};
use crate::types::PaxosMessage;
use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration}; // For retrying and timeout

pub async fn leader_main(leader_addr: &str, load_balancer_addr: &str, multicast_ip: &str) {
    // Bind to the real network interface IP
    let socket = UdpSocket::bind(leader_addr).await.unwrap();

    // Set multicast TTL (Time to Live) value to 1 to restrict the message to the local network
    socket.set_multicast_ttl_v4(1).unwrap();

    // Retry logic for registering with load balancer
    let lb_registration_message = format!("register:{}", leader_addr);
    let mut registered = false;
    while !registered {
        match socket
            .send_to(lb_registration_message.as_bytes(), load_balancer_addr)
            .await
        {
            Ok(_) => {
                println!(
                    "Leader registered with load balancer: {}",
                    load_balancer_addr
                );
                registered = true;
            }
            Err(e) => {
                println!(
                    "Failed to register with load balancer, retrying in 2 seconds: {}",
                    e
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }

    loop {
        let (message, _src_addr) = receive_message(&socket).await.unwrap();

        match message {
            PaxosMessage::ClientRequest {
                request_id,
                payload,
            } => {
                let original_message = String::from_utf8_lossy(&payload).to_string();
                println!("Leader received request from client: {}", original_message);

                // Multicast group address (e.g., 224.0.0.1)
                let multicast_group = Ipv4Addr::new(224, 0, 0, 1);
                let multicast_addr = SocketAddrV4::new(multicast_group, 8080);

                let broadcast_message = PaxosMessage::ClientRequest {
                    request_id,
                    payload: payload.clone(),
                };

                // Serialize and send the message to the multicast group
                let serialized_message = bincode::serialize(&broadcast_message).unwrap();
                match socket.send_to(&serialized_message, multicast_addr).await {
                    Ok(_) => {
                        println!(
                            "Leader multicast request to followers via address: {}",
                            multicast_addr
                        );
                    }
                    Err(e) => {
                        println!("Error sending multicast message: {}", e);
                    }
                }

                let mut acks = 0;
                let majority = 2; // Expecting majority acknowledgment

                // Wait for acknowledgment with timeout
                for _ in 0..majority {
                    match timeout(Duration::from_secs(2), receive_message(&socket)).await {
                        Ok(Ok((ack, _))) => {
                            if let PaxosMessage::FollowerAck { .. } = ack {
                                acks += 1;
                                println!("Leader received acknowledgment from a follower");
                            }
                        }
                        Ok(Err(e)) => {
                            println!("Error receiving acknowledgment from follower: {}", e);
                        }
                        Err(_) => {
                            println!("Timeout waiting for acknowledgment from followers");
                        }
                    }

                    if acks >= majority {
                        println!("Leader received majority acknowledgment, responding to load balancer at {}", load_balancer_addr);

                        let response = format!(
                            "Request ID: {}\nOriginal Message: {}\nAcknowledgments Received: {}\n",
                            request_id, original_message, acks
                        );
                        socket
                            .send_to(response.as_bytes(), load_balancer_addr)
                            .await
                            .unwrap();
                        break;
                    }
                }

                if acks < majority {
                    let response = format!(
                        "Request ID: {}\nNot enough acknowledgments to proceed (Received: {}, Majority: {}).",
                        request_id, acks, majority
                    );
                    socket
                        .send_to(response.as_bytes(), load_balancer_addr)
                        .await
                        .unwrap();
                }
            }
            _ => {}
        }
    }
}
