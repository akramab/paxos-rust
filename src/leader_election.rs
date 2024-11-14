use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, mem, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Config {
    #[arg(long)]
    node_id: u64,
    #[arg(short, long)]
    port: String,
}

type NodeId = u64;
type DataValue = String;

#[derive(Clone, Debug, Default)]
struct Receiver {
    pub last_round_number: u64,
    pub agreed_value: Option<Proposal>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PeerNode {
    pub node_id: u64,
    pub address: SocketAddr,
}

impl PeerNode {
    pub fn new(node_id: u64, address: SocketAddr) -> Self {
        Self { node_id, address }
    }
}

type DataStore = HashMap<NodeId, DataValue>;

#[derive(Clone, Debug)]
struct SharedState {
    node: PeerNode,
    network: Arc<Mutex<Vec<PeerNode>>>,
    receiver: Arc<Mutex<Receiver>>,
    initiator: Arc<Mutex<Initiator>>,
    data_store: Arc<Mutex<DataStore>>,
}

#[tokio::main]
async fn main() {
    let config = Config::parse();
    let port = config.port;
    let node_id = config.node_id;

    let node_address = format!("0.0.0.0:{}", port);

    println!("Launching node at: http://{}", node_address);

    let node = PeerNode::new(node_id, node_address.parse().unwrap());
    let state = SharedState {
        node,
        network: Arc::new(Mutex::new(Vec::new())),
        receiver: Arc::new(Mutex::new(Receiver::default())),
        initiator: Arc::new(Mutex::new(Initiator::new())),
        data_store: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/", get(fetch_node_info))
        .route("/info", get(display_state))
        .route("/ping", post(process_ping))
        .route("/connect", post(establish_connection))
        .route("/initiate", post(initiate_proposal))
        .route("/respond-prepare", post(respond_prepare))
        .route("/respond-accept", post(respond_accept))
        .route("/respond-learn", post(record_agreement))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(node_address).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn establish_connection(
    State(state): State<SharedState>,
    target: String,
) -> (StatusCode, String) {
    let PeerNode { node_id, address } = state.node;

    let mut payload = HashMap::new();
    payload.insert("node_id", node_id.to_string());
    payload.insert("address", address.to_string());

    let client = Client::new();
    let res = client
        .post(format!("http://0.0.0.0:{}/ping", target))
        .json(&payload)
        .send()
        .await;

    match res {
        Err(_) => todo!(),
        Ok(res) => {
            let is_error = res.status().is_client_error() || res.status().is_server_error();
            if is_error {
                return (StatusCode::BAD_REQUEST, res.text().await.unwrap());
            }

            let body_text = res.text().await.unwrap();
            let body: PingResponse = serde_json::from_str(body_text.as_str()).unwrap();

            let mut network = state.network.lock().await;

            let node_id: u64 = body.node_id.parse().unwrap();
            let address: SocketAddr = body.address.parse().unwrap();
            network.push(PeerNode { node_id, address });
            std::mem::drop(network);

            println!(
                "[/connect] Connected to new node: {} - ID: {}",
                address, node_id
            );

            (StatusCode::OK, format!("Connected to peer: {}!", target))
        }
    }
}

#[derive(Deserialize, Debug)]
struct PingResponse {
    pub node_id: String,
    pub address: String,
}

async fn process_ping(
    State(state): State<SharedState>,
    Json(body): Json<PingResponse>,
) -> (StatusCode, Json<HashMap<&'static str, String>>) {
    let node_id: u64 = body.node_id.parse().unwrap();
    if node_id == state.node.node_id {
        let mut response = HashMap::new();
        response.insert("error", String::from("Cannot connect to the same node!"));
        return (StatusCode::BAD_REQUEST, Json(response));
    }

    let mut network = state.network.lock().await;

    if network.iter().any(|peer| peer.node_id == node_id) {
        let mut response = HashMap::new();
        response.insert("error", String::from("Already connected to this node!"));
        return (StatusCode::BAD_REQUEST, Json(response));
    }

    network.push(PeerNode {
        node_id,
        address: body.address.parse().unwrap(),
    });
    std::mem::drop(network);

    println!("[/ping] State updated: {:?}", state);

    let mut response = HashMap::new();
    response.insert("node_id", state.node.node_id.to_string());
    response.insert("address", state.node.address.to_string());

    (StatusCode::OK, Json(response))
}

async fn fetch_node_info(State(state): State<SharedState>) -> (StatusCode, String) {
    println!("[/] Node info: {:?}", state);
    let info = serde_json::to_string(&state.node).unwrap();
    (StatusCode::OK, info)
}

async fn display_state(State(state): State<SharedState>) -> (StatusCode, ()) {
    println!("State info: {:?}", state);
    (StatusCode::OK, ())
}

async fn initiate_proposal(
    State(state): State<SharedState>,
    proposal: String,
) -> (StatusCode, String) {
    let mut initiator = state.initiator.lock().await;
    let ballot = match initiator.request_preparation(&state, proposal).await {
        Err(e) => return (StatusCode::BAD_REQUEST, e.clone()),
        Ok(ballot) => ballot,
    };

    match initiator.send_proposal(&state, &ballot).await {
        Err(e) => (StatusCode::BAD_REQUEST, e),
        Ok(_) => {
            let client = Client::new();
            let mut data_store = state.data_store.lock().await;
            let network = state.network.lock().await;

            let reqs = network.iter().map(|peer| {
                client
                    .post(format!("http://{}/respond-learn", peer.address))
                    .json(&ballot)
                    .send()
            });

            futures::future::join_all(reqs).await;

            data_store.insert(ballot.node_id, ballot.value.unwrap_or_default());

            (StatusCode::OK, String::from("Proposal agreed by majority!"))
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct PrepareResponsePayload {
    error: Option<String>,
    value: Option<ProposalBallot>,
}

async fn respond_prepare(
    State(state): State<SharedState>,
    proposal_id: String,
) -> (StatusCode, Json<PrepareResponsePayload>) {
    let proposal_id: u64 = proposal_id.parse().unwrap();
    let mut receiver = state.receiver.lock().await;

    if proposal_id <= receiver.last_round_number {
        let response = PrepareResponsePayload {
            error: Some(String::from(
                "Proposal ID is below the last accepted round number",
            )),
            value: None,
        };
        return (StatusCode::BAD_REQUEST, Json(response));
    }

    if receiver.agreed_value.is_some() && receiver.last_round_number == proposal_id {
        println!(
            "[/respond-prepare] Node {} has existing value: {:?}",
            state.node.node_id, receiver.agreed_value
        );
        let response = PrepareResponsePayload {
            error: None,
            value: Some(ProposalBallot {
                id: proposal_id,
                value: receiver.agreed_value.clone(),
            }),
        };
        return (StatusCode::OK, Json(response));
    }

    receiver.last_round_number = proposal_id;

    println!(
        "[/respond-prepare] New round number set to: {}",
        proposal_id
    );

    let response = PrepareResponsePayload {
        error: None,
        value: Some(ProposalBallot {
            id: proposal_id,
            value: None,
        }),
    };

    (StatusCode::OK, Json(response))
}

#[derive(Serialize, Deserialize, Debug)]
struct AcceptResponsePayload {
    error: Option<String>,
    value: Option<ProposalBallot>,
}

async fn respond_accept(
    State(state): State<SharedState>,
    proposal: Json<ProposalBallot>,
) -> (StatusCode, Json<AcceptResponsePayload>) {
    println!(
        "[/respond-accept] Node {} received new proposal: {:?}",
        state.node.node_id, proposal
    );

    let mut receiver = state.receiver.lock().await;
    if receiver.last_round_number != proposal.id {
        println!(
            "[/respond-accept] Node {} received mismatched proposal ID: {}",
            state.node.node_id, proposal.id
        );
        let response = AcceptResponsePayload {
            error: Some(String::from("Proposal ID mismatch")),
            value: None,
        };
        return (StatusCode::BAD_REQUEST, Json(response));
    }

    receiver.agreed_value = proposal.value.clone();

    let response = AcceptResponsePayload {
        error: None,
        value: Some(ProposalBallot {
            id: proposal.id,
            value: proposal.value.clone(),
        }),
    };

    (StatusCode::OK, Json(response))
}

async fn record_agreement(
    State(state): State<SharedState>,
    ballot: Json<ProposalBallot>,
) -> (StatusCode, ()) {
    let mut data_store = state.data_store.lock().await;

    match &ballot.value {
        None => data_store.insert(ballot.id, String::from("")),
        Some(v) => data_store.insert(ballot.id, v.clone()),
    };

    println!(
        "[/respond-learn] Node {} recorded a new value: {:?}",
        state.node.node_id, ballot.value
    );

    reset_round(&state).await;

    (StatusCode::OK, ())
}

#[derive(Serialize, Deserialize, Debug)]
struct ProposalBallot {
    pub id: u64,
    pub value: Option<String>,
}

#[derive(Clone, Debug)]
struct Initiator {
    pub round_id: u64,
}

type Proposal = DataValue;

impl Initiator {
    pub fn new() -> Self {
        Self { round_id: 0 }
    }

    pub async fn request_preparation(
        &mut self,
        state: &SharedState,
        proposal: String,
    ) -> Result<ProposalBallot, String> {
        let client = Client::new();

        self.round_id += 1;

        let network = state.network.lock().await;
        let reqs = network.iter().map(|peer| {
            client
                .post(format!("http://{}/respond-prepare", peer.address))
                .json(&self.round_id)
                .send()
        });

        let responses = futures::future::join_all(reqs).await;

        let mut accepted = Vec::with_capacity(responses.len());

        for response in responses.into_iter().flatten() {
            accepted.push(response.json::<PrepareResponsePayload>().await.unwrap());
        }

        let quorum = (network.len() / 2) + 1;

        if accepted.len() < quorum {
            return Err(String::from("Insufficient promises for proposal"));
        }

        let chosen = accepted
            .into_iter()
            .filter(|response| response.value.is_some())
            .max_by_key(|response| response.value.as_ref().map(|ballot| ballot.id).unwrap_or(0));

        let final_value = chosen
            .and_then(|resp| resp.value.map(|v| v.value))
            .unwrap_or(Some(proposal));

        Ok(ProposalBallot {
            id: self.round_id,
            value: final_value,
        })
    }

    pub async fn send_proposal(
        &self,
        state: &SharedState,
        proposal: &ProposalBallot,
    ) -> Result<(), String> {
        let client = Client::new();

        let network = state.network.lock().await;

        let reqs = network.iter().map(|peer| {
            client
                .post(format!("http://{}/respond-accept", peer.address))
                .json(&proposal)
                .send()
        });

        let responses = futures::future::join_all(reqs).await;

        let approved = responses.into_iter().filter_map(|r| r.ok()).count();

        let quorum = (network.len() / 2) + 1;

        if approved < quorum {
            return Err(String::from("Proposal rejected by majority"));
        }

        Ok(())
    }
}

async fn reset_round(state: &SharedState) {
    let mut receiver = state.receiver.lock().await;
    receiver.last_round_number = 0;
    receiver.agreed_value = None;

    let mut initiator = state.initiator.lock().await;
    initiator.round_id = 0;

    println!("[reset_round] Reset completed!");
}
