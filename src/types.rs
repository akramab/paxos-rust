use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowerRegistration {
    pub follower_addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaxosMessage {
    ClientRequest { request_id: u64, payload: Vec<u8> },
    FollowerAck { request_id: u64 },
    RegisterFollower(FollowerRegistration),
}
