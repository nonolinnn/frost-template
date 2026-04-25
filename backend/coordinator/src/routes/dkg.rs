use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::dkg::{DkgRoundResponse, DkgStartResponse, DkgStatusResponse, NodeRoundStatus};
use crate::AppState;

/// Build the DKG sub-router.
///
/// Mounted at `/api/dkg` in the top-level router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/start", post(start_dkg))
        .route("/status", get(get_dkg_status))
        .route("/round/{round}/node/{node_id}", post(execute_round))
}

/// Valid node IDs.
const NODE_IDS: [&str; 2] = ["node-a", "node-b"];

/// Get the other node's ID.
fn other_node_id(node_id: &str) -> &'static str {
    if node_id == "node-a" {
        "node-b"
    } else {
        "node-a"
    }
}

/// Build per-node round status map from DB round states.
fn build_node_status(
    round_states: &[crate::models::dkg::DkgRoundStateRow],
) -> HashMap<String, NodeRoundStatus> {
    let mut nodes: HashMap<String, NodeRoundStatus> = HashMap::new();

    // Initialize all nodes with pending
    for nid in &NODE_IDS {
        nodes.insert(
            nid.to_string(),
            NodeRoundStatus {
                round1: "pending".to_string(),
                round2: "pending".to_string(),
                round3: "pending".to_string(),
            },
        );
    }

    // Fill in actual statuses from DB
    for rs in round_states {
        if let Some(entry) = nodes.get_mut(&rs.node_id) {
            let status_str = rs.status.clone();
            match rs.round {
                1 => entry.round1 = status_str,
                2 => entry.round2 = status_str,
                3 => entry.round3 = status_str,
                _ => {}
            }
        }
    }

    nodes
}

/// Format a timestamp for API responses.
fn format_ts(ts: time::OffsetDateTime) -> String {
    ts.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| ts.to_string())
}

/// `POST /api/dkg/start` -- initialize a new DKG session.
async fn start_dkg(
    State(state): State<AppState>,
) -> AppResult<(StatusCode, Json<DkgStartResponse>)> {
    // Check for existing sessions
    let existing = db::dkg::get_active_session(&state.pool).await?;
    if let Some(session) = existing {
        match session.status.as_str() {
            "complete" => return Err(AppError::DkgAlreadyExists),
            "initialized" | "in_progress" => return Err(AppError::DkgInProgress),
            _ => {} // failed sessions are OK to start over
        }
    }

    let session = db::dkg::create_session(&state.pool).await?;
    let round_states = db::dkg::get_round_states(&state.pool, session.id).await?;
    let nodes = build_node_status(&round_states);

    Ok((
        StatusCode::CREATED,
        Json(DkgStartResponse {
            session_id: session.id,
            status: session.status,
            created_at: format_ts(session.created_at),
            nodes,
        }),
    ))
}

/// `GET /api/dkg/status` -- get current DKG session status.
async fn get_dkg_status(State(state): State<AppState>) -> AppResult<Json<DkgStatusResponse>> {
    let session = db::dkg::get_active_session(&state.pool).await?;

    match session {
        Some(session) => {
            let round_states = db::dkg::get_round_states(&state.pool, session.id).await?;
            let nodes = build_node_status(&round_states);

            Ok(Json(DkgStatusResponse {
                session_id: Some(session.id),
                status: session.status,
                created_at: Some(format_ts(session.created_at)),
                completed_at: session.completed_at.map(format_ts),
                group_public_key: session.group_public_key,
                nodes,
            }))
        }
        None => Ok(Json(DkgStatusResponse {
            session_id: None,
            status: "not_started".to_string(),
            created_at: None,
            completed_at: None,
            group_public_key: None,
            nodes: HashMap::new(),
        })),
    }
}

/// `POST /api/dkg/round/{round}/node/{node_id}` -- trigger a DKG round on a node.
///
/// The Coordinator calls the target node's round endpoint, passing any required
/// data from previous rounds, then stores the result.
async fn execute_round(
    State(state): State<AppState>,
    Path((round, node_id)): Path<(i16, String)>,
) -> AppResult<Json<DkgRoundResponse>> {
    // Validate inputs
    if !(1..=3).contains(&round) {
        return Err(AppError::InvalidRound { round });
    }
    if !NODE_IDS.contains(&node_id.as_str()) {
        return Err(AppError::InvalidNodeId { node_id });
    }

    // Get active session
    let session = db::dkg::get_active_session(&state.pool)
        .await?
        .ok_or(AppError::DkgSessionNotFound)?;

    // Must not be complete or failed
    if session.status == "complete" {
        return Err(AppError::DkgAlreadyExists);
    }

    let session_id = session.id;

    // Check if this round is already complete for this node
    let round_state = db::dkg::get_round_state(&state.pool, session_id, &node_id, round)
        .await?
        .ok_or(AppError::DkgSessionNotFound)?;

    if round_state.status == "complete" {
        return Err(AppError::RoundAlreadyComplete {
            node_id: node_id.clone(),
            round,
        });
    }

    let other_id = other_node_id(&node_id);

    // Get the node's base URL
    let node_url = state
        .config
        .node_url(&node_id)
        .ok_or_else(|| AppError::InvalidNodeId {
            node_id: node_id.clone(),
        })?
        .to_string();

    match round {
        1 => execute_round1(&state, session_id, &node_id, &node_url).await?,
        2 => execute_round2(&state, session_id, &node_id, other_id, &node_url).await?,
        3 => execute_round3(&state, session_id, &node_id, other_id, &node_url).await?,
        _ => unreachable!(),
    }

    // Update session status to in_progress if it was initialized
    if session.status == "initialized" {
        db::dkg::update_session_status(&state.pool, session_id, "in_progress", None).await?;
    }

    // Reload round states to check if DKG is complete
    let round_states = db::dkg::get_round_states(&state.pool, session_id).await?;
    let nodes = build_node_status(&round_states);

    // Check if all nodes completed Round 3
    let all_round3_complete = NODE_IDS.iter().all(|nid| {
        nodes
            .get(*nid)
            .map(|ns| ns.round3 == "complete")
            .unwrap_or(false)
    });

    let mut dkg_complete = None;
    let mut group_public_key = None;

    if all_round3_complete {
        // Retrieve the group public key from the round 3 output
        // (stored as verifying_share in output_package, but we stored group_public_key)
        // Look at the round3 output_package from any node -- it contains the group_public_key
        let r3_state = round_states
            .iter()
            .find(|rs| rs.round == 3 && rs.status == "complete")
            .ok_or_else(|| AppError::Internal {
                message: "Round 3 complete but no output found".to_string(),
            })?;

        // The output_package stores { "group_public_key": "...", "verifying_share": "..." }
        let output = r3_state
            .output_package
            .as_ref()
            .ok_or_else(|| AppError::Internal {
                message: "Round 3 output_package is null".to_string(),
            })?;

        let gpk = output
            .get("group_public_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Internal {
                message: "group_public_key not found in round 3 output".to_string(),
            })?
            .to_string();

        db::dkg::update_session_status(&state.pool, session_id, "complete", Some(&gpk)).await?;

        dkg_complete = Some(true);
        group_public_key = Some(gpk);
    }

    Ok(Json(DkgRoundResponse {
        session_id,
        node_id: node_id.clone(),
        round,
        status: "complete".to_string(),
        dkg_complete,
        group_public_key,
        nodes,
    }))
}

/// Execute Round 1: call the node's /api/dkg/round1 endpoint.
async fn execute_round1(
    state: &AppState,
    session_id: uuid::Uuid,
    node_id: &str,
    node_url: &str,
) -> AppResult<()> {
    let url = format!("{node_url}/api/dkg/round1");
    let body = serde_json::json!({ "session_id": session_id });

    let response = state
        .http_client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|_e| AppError::NodeUnavailable {
            node_id: node_id.to_string(),
        })?;

    if !response.status().is_success() {
        let err_text = response.text().await.unwrap_or_default();
        return Err(AppError::NodeError {
            node_id: node_id.to_string(),
            message: err_text,
        });
    }

    let resp: serde_json::Value = response.json().await.map_err(|e| AppError::NodeError {
        node_id: node_id.to_string(),
        message: format!("Failed to parse round1 response: {e}"),
    })?;

    // Store the round1_package as the output_package for this round
    let round1_package =
        resp.get("round1_package")
            .cloned()
            .ok_or_else(|| AppError::NodeError {
                node_id: node_id.to_string(),
                message: "round1_package missing from response".to_string(),
            })?;

    db::dkg::complete_round(&state.pool, session_id, node_id, 1, Some(round1_package)).await?;

    Ok(())
}

/// Execute Round 2: gather the other node's Round 1 package, call the target
/// node's /api/dkg/round2 endpoint.
async fn execute_round2(
    state: &AppState,
    session_id: uuid::Uuid,
    node_id: &str,
    other_id: &str,
    node_url: &str,
) -> AppResult<()> {
    // Precondition: this node must have completed Round 1
    let self_r1 = db::dkg::get_round_state(&state.pool, session_id, node_id, 1)
        .await?
        .ok_or(AppError::DkgSessionNotFound)?;
    if self_r1.status != "complete" {
        return Err(AppError::RoundPrecondition {
            message: format!("{node_id} has not completed Round 1"),
        });
    }

    // Precondition: the other node must have completed Round 1
    let other_r1 = db::dkg::get_round_state(&state.pool, session_id, other_id, 1)
        .await?
        .ok_or(AppError::DkgSessionNotFound)?;
    if other_r1.status != "complete" {
        return Err(AppError::RoundPrecondition {
            message: format!("{other_id} has not completed Round 1"),
        });
    }

    // Build the round1_packages map with the other node's package
    let other_r1_package = other_r1.output_package.ok_or_else(|| AppError::Internal {
        message: format!("{other_id} Round 1 output_package is null"),
    })?;

    let mut round1_packages = serde_json::Map::new();
    round1_packages.insert(other_id.to_string(), other_r1_package);

    let url = format!("{node_url}/api/dkg/round2");
    let body = serde_json::json!({
        "session_id": session_id,
        "round1_packages": round1_packages,
    });

    let response = state
        .http_client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|_e| AppError::NodeUnavailable {
            node_id: node_id.to_string(),
        })?;

    if !response.status().is_success() {
        let err_text = response.text().await.unwrap_or_default();
        return Err(AppError::NodeError {
            node_id: node_id.to_string(),
            message: err_text,
        });
    }

    let resp: serde_json::Value = response.json().await.map_err(|e| AppError::NodeError {
        node_id: node_id.to_string(),
        message: format!("Failed to parse round2 response: {e}"),
    })?;

    // Store the round2_package as the output for this round
    let round2_package =
        resp.get("round2_package")
            .cloned()
            .ok_or_else(|| AppError::NodeError {
                node_id: node_id.to_string(),
                message: "round2_package missing from response".to_string(),
            })?;

    db::dkg::complete_round(&state.pool, session_id, node_id, 2, Some(round2_package)).await?;

    Ok(())
}

/// Execute Round 3: gather Round 1 and Round 2 packages from the other node,
/// call the target node's /api/dkg/round3 endpoint. Store the group public key.
async fn execute_round3(
    state: &AppState,
    session_id: uuid::Uuid,
    node_id: &str,
    other_id: &str,
    node_url: &str,
) -> AppResult<()> {
    // Precondition: this node must have completed Round 2
    let self_r2 = db::dkg::get_round_state(&state.pool, session_id, node_id, 2)
        .await?
        .ok_or(AppError::DkgSessionNotFound)?;
    if self_r2.status != "complete" {
        return Err(AppError::RoundPrecondition {
            message: format!("{node_id} has not completed Round 2"),
        });
    }

    // Precondition: the other node must have completed Round 2
    let other_r2 = db::dkg::get_round_state(&state.pool, session_id, other_id, 2)
        .await?
        .ok_or(AppError::DkgSessionNotFound)?;
    if other_r2.status != "complete" {
        return Err(AppError::RoundPrecondition {
            message: format!("{other_id} has not completed Round 2"),
        });
    }

    // Gather the other node's Round 1 package
    let other_r1 = db::dkg::get_round_state(&state.pool, session_id, other_id, 1)
        .await?
        .ok_or(AppError::DkgSessionNotFound)?;
    let other_r1_package = other_r1.output_package.ok_or_else(|| AppError::Internal {
        message: format!("{other_id} Round 1 output_package is null"),
    })?;

    // Gather the other node's Round 2 package
    let other_r2_package = other_r2.output_package.ok_or_else(|| AppError::Internal {
        message: format!("{other_id} Round 2 output_package is null"),
    })?;

    let mut round1_packages = serde_json::Map::new();
    round1_packages.insert(other_id.to_string(), other_r1_package);

    let mut round2_packages = serde_json::Map::new();
    round2_packages.insert(other_id.to_string(), other_r2_package);

    let url = format!("{node_url}/api/dkg/round3");
    let body = serde_json::json!({
        "session_id": session_id,
        "round1_packages": round1_packages,
        "round2_packages": round2_packages,
    });

    let response = state
        .http_client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|_e| AppError::NodeUnavailable {
            node_id: node_id.to_string(),
        })?;

    if !response.status().is_success() {
        let err_text = response.text().await.unwrap_or_default();
        return Err(AppError::NodeError {
            node_id: node_id.to_string(),
            message: err_text,
        });
    }

    let resp: serde_json::Value = response.json().await.map_err(|e| AppError::NodeError {
        node_id: node_id.to_string(),
        message: format!("Failed to parse round3 response: {e}"),
    })?;

    // Store the round3 output (group_public_key + verifying_share) as output_package
    let group_public_key = resp
        .get("group_public_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::NodeError {
            node_id: node_id.to_string(),
            message: "group_public_key missing from response".to_string(),
        })?
        .to_string();

    let verifying_share = resp
        .get("verifying_share")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let output = serde_json::json!({
        "group_public_key": group_public_key,
        "verifying_share": verifying_share,
    });

    db::dkg::complete_round(&state.pool, session_id, node_id, 3, Some(output)).await?;

    Ok(())
}
