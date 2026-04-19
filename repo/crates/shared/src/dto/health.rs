//! Health + readiness DTOs (endpoints S1 and S2).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadyResponse {
    pub status: &'static str,
    pub db: bool,
}
