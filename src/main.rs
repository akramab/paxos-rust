mod follower;
mod leader;
mod load_balancer;
mod network;
mod types;

#[tokio::main]
async fn main() {
    let role = std::env::args().nth(1).expect("No role provided");

    if role == "leader" {
        let leader_addr = "127.0.0.1:8080"; // Leader address
        let load_balancer_addr = "127.0.0.1:8000"; // Load balancer address
        leader::leader_main(leader_addr, load_balancer_addr).await;
    } else if role == "follower" {
        let follower_addr = std::env::args()
            .nth(2)
            .expect("No follower address provided");
        let leader_addr = std::env::args().nth(3).expect("No leader address provided");
        let load_balancer_addr = std::env::args()
            .nth(4)
            .expect("No load balancer address provided");
        follower::follower_main(&follower_addr, &leader_addr, &load_balancer_addr).await;
    } else if role == "load_balancer" {
        let mut lb = load_balancer::LoadBalancer::new(); // Declare lb as mutable
        lb.listen_and_route("127.0.0.1:8000").await; // Call listen_and_route with mutable reference
    } else {
        panic!("Invalid role! Use 'leader', 'follower', or 'load_balancer'.");
    }
}
