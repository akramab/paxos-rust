use crate::network::{receive_message, send_message};
use crate::types::{FollowerRegistration, PaxosMessage};
use tokio::net::UdpSocket;
use tokio::time::{sleep, Duration};

pub async fn follower_main(follower_addr: &str, leader_addr: &str, load_balancer_addr: &str) {
    let socket = UdpSocket::bind(follower_addr).await.unwrap();

    // Register with the leader
    let registration_message = PaxosMessage::RegisterFollower(FollowerRegistration {
        follower_addr: follower_addr.to_string(),
    });
    send_message(&socket, registration_message, leader_addr)
        .await
        .unwrap();
    println!("Follower registered with leader: {}", leader_addr);

    // Retry logic for registering with load balancer
    let lb_registration_message = format!("register:{}", follower_addr);
    let mut registered = false;
    while !registered {
        match socket
            .send_to(lb_registration_message.as_bytes(), load_balancer_addr)
            .await
        {
            Ok(_) => {
                println!(
                    "Follower registered with load balancer: {}",
                    load_balancer_addr
                );
                registered = true; // Registration successful
            }
            Err(e) => {
                println!(
                    "Failed to register with load balancer, retrying in 2 seconds: {}",
                    e
                );
                sleep(Duration::from_secs(2)).await; // Retry after 2 seconds
            }
        }
    }

    // Continue listening for requests from the leader or client
    loop {
        let (message, src_addr) = receive_message(&socket).await.unwrap();
        match message {
            PaxosMessage::ClientRequest {
                request_id,
                payload,
            } => {
                if src_addr != leader_addr {
                    println!("Follower received client request. Forwarding to leader.");
                    send_message(
                        &socket,
                        PaxosMessage::ClientRequest {
                            request_id,
                            payload,
                        },
                        leader_addr,
                    )
                    .await
                    .unwrap();
                } else {
                    println!("Follower received request from leader: {:?}", payload);
                    let ack = PaxosMessage::FollowerAck { request_id };
                    send_message(&socket, ack, &leader_addr).await.unwrap();
                    println!("Follower acknowledged request ID: {}", request_id);
                }
            }
            _ => {}
        }
    }
}
