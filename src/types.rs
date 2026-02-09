use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessRequest {
    pub id: String,
    pub payload: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessResponse {
    pub id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerRequest {
    pub id: String,
    pub payload: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerResponse {
    pub success: bool,
    pub message: String,
}
