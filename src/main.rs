// src/main.rs
mod follower;
mod leader;
mod network;
mod types;

#[tokio::main]
async fn main() {
    let role = std::env::args().nth(1).expect("No role provided");

    if role == "leader" {
        let leader_addr = "127.0.0.1:8080"; // Leader address
        leader::leader_main(leader_addr).await;
    } else if role == "follower" {
        let follower_addr = std::env::args()
            .nth(2)
            .expect("No follower address provided");
        let leader_addr = std::env::args().nth(3).expect("No leader address provided");
        follower::follower_main(&follower_addr, &leader_addr).await;
    } else {
        panic!("Invalid role! Use 'leader' or 'follower'.");
    }
}
