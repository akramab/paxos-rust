// src/follower.rs
use crate::network::{receive_message, send_message};
use crate::types::{FollowerRegistration, PaxosMessage};
use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::net::UdpSocket;

pub async fn follower_main(
    follower_addr: &str,
    leader_addr: &str,
    load_balancer_addr: &str,
    multicast_ip: &str,
) {
    let socket = UdpSocket::bind(follower_addr).await.unwrap();

    // Register follower with leader
    let registration_message = PaxosMessage::RegisterFollower(FollowerRegistration {
        follower_addr: follower_addr.to_string(),
    });
    send_message(&socket, registration_message, leader_addr)
        .await
        .unwrap();

    // Register follower with load balancer
    let lb_registration_message = format!("register:{}", follower_addr);
    socket
        .send_to(lb_registration_message.as_bytes(), load_balancer_addr)
        .await
        .unwrap();

    // Join the multicast group
    let multicast_group = Ipv4Addr::new(224, 0, 0, 1);
    let local_interface = Ipv4Addr::UNSPECIFIED; // Bind to any available interface
    socket
        .join_multicast_v4(multicast_group, local_interface)
        .unwrap();

    loop {
        // Receive multicast messages from leader
        let (message, _src_addr) = receive_message(&socket).await.unwrap();

        if let PaxosMessage::ClientRequest {
            request_id,
            payload,
        } = message
        {
            println!("Follower received request from leader: {:?}", payload);

            // Send acknowledgment back to the leader
            let ack_message = PaxosMessage::FollowerAck { request_id };
            send_message(&socket, ack_message, leader_addr)
                .await
                .unwrap();
            println!("Follower acknowledged request ID: {}", request_id);
        }
    }
}
