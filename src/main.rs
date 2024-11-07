mod follower;
mod leader;
mod network;
mod types;

use tokio::sync::mpsc;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let role = args.get(1).expect("No role provided (leader or follower)");
    let node_id = Uuid::new_v4();

    let (ack_tx, ack_rx) = mpsc::channel(32); // Create both sender and receiver

    match role.as_str() {
        "leader" => {
            let leader_addr = "127.0.0.1:8080";
            leader::leader_main(node_id, leader_addr, ack_rx).await; // Pass receiver to the leader
        }
        "follower" => {
            let follower_addr = args.get(2).expect("No follower address provided");
            let leader_addr = "127.0.0.1:8080"; // Static leader address for followers
            follower::follower_main(node_id, follower_addr, leader_addr).await;
        }
        _ => {
            eprintln!("Unknown role! Use 'leader' or 'follower'");
        }
    }
}
