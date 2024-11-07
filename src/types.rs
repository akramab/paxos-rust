// src/types.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub enum PaxosMessage {
    RegisterFollower {
        follower_addr: String, // Define the follower address field
    },
    ClientRequest {
        request_id: Uuid,
        payload: Vec<u8>,
    },
    FollowerAck {
        request_id: Uuid,
    },
}
