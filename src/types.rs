use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PaxosMessage {
    RegisterFollower { follower_addr: String },
    ClientRequest { request_id: Uuid, payload: Vec<u8> },
    FollowerAck { request_id: Uuid },
    Heartbeat { leader_id: Uuid },
    Election { candidate_id: Uuid },
    LeaderAnnouncement { new_leader_id: Uuid },
}
