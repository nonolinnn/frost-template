//! Route modules for the TSS Node service.
//!
//! Each sub-module defines an axum `Router` that is nested under the
//! appropriate path prefix in `main.rs`.

pub mod dkg;
pub mod signing;

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::AppState;

/// `GET /health` -- basic health check.
pub async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "tss-node",
        "node_id": state.config.node_id,
    }))
}
