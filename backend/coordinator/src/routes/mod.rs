//! Route modules for the Coordinator service.
//!
//! Each sub-module defines an axum `Router` that is nested under the
//! appropriate path prefix in `main.rs`.

pub mod dkg;
pub mod signing;
pub mod wallets;

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::AppState;

/// `GET /health` — basic health check.
pub async fn health(State(_state): State<AppState>) -> Json<Value> {
    Json(json!({ "status": "ok", "service": "coordinator" }))
}
