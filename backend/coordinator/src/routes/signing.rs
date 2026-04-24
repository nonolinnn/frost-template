use std::collections::{BTreeMap, HashMap};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use frost_ed25519::{self as frost, Identifier};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature as SolSignature;
use solana_sdk::transaction::Transaction;
use solana_system_interface::instruction::transfer as system_transfer;

use crate::db;
use crate::derivation;
use crate::error::{AppError, AppResult};
use crate::models::signing::{
    AggregateResponse, CreateSigningRequest, SigningNodeRoundStatus, SigningRequestListResponse,
    SigningRequestResponse, SigningRoundResponse,
};
use crate::AppState;

/// Build the signing-requests sub-router.
///
/// Mounted at `/api/signing-requests` in the top-level router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_signing_request).get(list_signing_requests))
        .route("/{id}", get(get_signing_request))
        .route("/{id}/round/{round}/node/{node_id}", post(execute_round))
        .route("/{id}/aggregate", post(aggregate))
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

/// Map a node ID string to a FROST Identifier.
fn node_id_to_identifier(node_id: &str) -> Result<Identifier, AppError> {
    Identifier::derive(node_id.as_bytes()).map_err(|e| AppError::Internal {
        message: format!("Failed to derive FROST identifier for {node_id}: {e}"),
    })
}

/// Format a timestamp for API responses.
fn format_ts(ts: time::OffsetDateTime) -> String {
    ts.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| ts.to_string())
}

/// Build per-node signing round status map from DB round states.
fn build_node_status(
    round_states: &[crate::models::signing::SigningRoundStateRow],
) -> HashMap<String, SigningNodeRoundStatus> {
    let mut nodes: HashMap<String, SigningNodeRoundStatus> = HashMap::new();

    // Initialize all nodes with pending
    for nid in &NODE_IDS {
        nodes.insert(
            nid.to_string(),
            SigningNodeRoundStatus {
                round1: "pending".to_string(),
                round2: "pending".to_string(),
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
                _ => {}
            }
        }
    }

    nodes
}

/// Build a `SigningRequestResponse` from a DB row and round states.
fn build_signing_request_response(
    row: &crate::models::signing::SigningRequestRow,
    round_states: &[crate::models::signing::SigningRoundStateRow],
    sender_address: &str,
) -> SigningRequestResponse {
    let nodes = build_node_status(round_states);

    let explorer_url = row.tx_signature.as_ref().map(|sig| {
        format!(
            "https://explorer.solana.com/tx/{sig}?cluster=devnet"
        )
    });

    SigningRequestResponse {
        id: row.id,
        wallet_index: row.wallet_index,
        sender_address: sender_address.to_string(),
        recipient: row.recipient.clone(),
        amount_lamports: row.amount_lamports,
        status: row.status.clone(),
        created_at: format_ts(row.created_at),
        updated_at: Some(format_ts(row.updated_at)),
        tx_signature: row.tx_signature.clone(),
        explorer_url,
        error_message: row.error_message.clone(),
        nodes,
    }
}

/// `POST /api/signing-requests` -- create a new signing request.
async fn create_signing_request(
    State(state): State<AppState>,
    Json(body): Json<CreateSigningRequest>,
) -> AppResult<(StatusCode, Json<SigningRequestResponse>)> {
    // Validate amount
    if body.amount_lamports <= 0 {
        return Err(AppError::InvalidAmount);
    }

    // Validate recipient as a valid Base58 Solana address
    let _recipient_pubkey = bs58::decode(&body.recipient)
        .into_vec()
        .map_err(|_| AppError::InvalidRecipient)
        .and_then(|bytes| {
            if bytes.len() == 32 {
                Ok(())
            } else {
                Err(AppError::InvalidRecipient)
            }
        })?;

    // Precondition: DKG must be complete
    let _session = db::dkg::get_completed_session(&state.pool)
        .await?
        .ok_or(AppError::DkgNotComplete)?;

    // Precondition: wallet must exist
    let wallet = db::wallet::get_wallet_by_index(&state.pool, body.wallet_index)
        .await?
        .ok_or(AppError::WalletNotFound {
            index: body.wallet_index,
        })?;

    // Create the signing request in DB
    let row = db::signing::create_signing_request(
        &state.pool,
        body.wallet_index,
        &body.recipient,
        body.amount_lamports,
    )
    .await?;

    let round_states =
        db::signing::get_signing_round_states(&state.pool, row.id).await?;

    let response = build_signing_request_response(&row, &round_states, &wallet.address);

    tracing::info!(
        id = %row.id,
        wallet_index = body.wallet_index,
        recipient = %body.recipient,
        amount_lamports = body.amount_lamports,
        "Signing request created"
    );

    Ok((StatusCode::CREATED, Json(response)))
}

/// `GET /api/signing-requests` -- list all signing requests.
async fn list_signing_requests(
    State(state): State<AppState>,
) -> AppResult<Json<SigningRequestListResponse>> {
    let rows = db::signing::list_signing_requests(&state.pool).await?;

    let mut signing_requests = Vec::with_capacity(rows.len());
    for row in &rows {
        let round_states =
            db::signing::get_signing_round_states(&state.pool, row.id).await?;

        // Look up wallet address
        let wallet = db::wallet::get_wallet_by_index(&state.pool, row.wallet_index).await?;
        let sender_address = wallet
            .map(|w| w.address)
            .unwrap_or_else(|| "unknown".to_string());

        signing_requests.push(build_signing_request_response(
            row,
            &round_states,
            &sender_address,
        ));
    }

    Ok(Json(SigningRequestListResponse { signing_requests }))
}

/// `GET /api/signing-requests/{id}` -- get a specific signing request.
async fn get_signing_request(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> AppResult<Json<SigningRequestResponse>> {
    let row = db::signing::get_signing_request(&state.pool, id)
        .await?
        .ok_or(AppError::SigningRequestNotFound { id })?;

    let round_states =
        db::signing::get_signing_round_states(&state.pool, row.id).await?;

    let wallet = db::wallet::get_wallet_by_index(&state.pool, row.wallet_index).await?;
    let sender_address = wallet
        .map(|w| w.address)
        .unwrap_or_else(|| "unknown".to_string());

    Ok(Json(build_signing_request_response(
        &row,
        &round_states,
        &sender_address,
    )))
}

/// `POST /api/signing-requests/{id}/round/{round}/node/{node_id}` -- trigger signing round.
async fn execute_round(
    State(state): State<AppState>,
    Path((id, round, node_id)): Path<(uuid::Uuid, i16, String)>,
) -> AppResult<Json<SigningRoundResponse>> {
    // Validate inputs
    if !(1..=2).contains(&round) {
        return Err(AppError::InvalidSigningRound { round });
    }
    if !NODE_IDS.contains(&node_id.as_str()) {
        return Err(AppError::InvalidNodeId {
            node_id: node_id.clone(),
        });
    }

    // Load signing request
    let signing_request = db::signing::get_signing_request(&state.pool, id)
        .await?
        .ok_or(AppError::SigningRequestNotFound { id })?;

    // Check terminal states
    match signing_request.status.as_str() {
        "confirmed" | "failed" | "broadcasted" | "aggregating" => {
            return Err(AppError::InvalidStatus);
        }
        _ => {}
    }

    // Check if this round is already complete for this node
    let round_state =
        db::signing::get_signing_round_state(&state.pool, id, &node_id, round)
            .await?
            .ok_or(AppError::SigningRequestNotFound { id })?;

    if round_state.status == "complete" {
        return Err(AppError::RoundAlreadyComplete {
            node_id: node_id.clone(),
            round,
        });
    }

    // Get the node's base URL
    let node_url = state
        .config
        .node_url(&node_id)
        .ok_or_else(|| AppError::InvalidNodeId {
            node_id: node_id.clone(),
        })?
        .to_string();

    match round {
        1 => {
            execute_signing_round1(
                &state,
                &signing_request,
                &node_id,
                &node_url,
            )
            .await?
        }
        2 => {
            execute_signing_round2(
                &state,
                &signing_request,
                &node_id,
                &node_url,
            )
            .await?
        }
        _ => unreachable!(),
    }

    // Determine new signing request status
    let round_states =
        db::signing::get_signing_round_states(&state.pool, id).await?;
    let nodes = build_node_status(&round_states);

    let new_status = compute_signing_status(&nodes);

    // Update signing request status if it changed
    if new_status != signing_request.status {
        db::signing::update_signing_request_status(&state.pool, id, &new_status).await?;
    }

    Ok(Json(SigningRoundResponse {
        signing_request_id: id,
        node_id: node_id.clone(),
        round,
        status: "complete".to_string(),
        signing_request_status: new_status,
        nodes,
    }))
}

/// Compute the overall signing request status from the per-node round statuses.
fn compute_signing_status(
    nodes: &HashMap<String, SigningNodeRoundStatus>,
) -> String {
    let all_round1_complete = NODE_IDS.iter().all(|nid| {
        nodes
            .get(*nid)
            .map(|ns| ns.round1 == "complete")
            .unwrap_or(false)
    });
    let any_round1_complete = NODE_IDS.iter().any(|nid| {
        nodes
            .get(*nid)
            .map(|ns| ns.round1 == "complete")
            .unwrap_or(false)
    });
    let all_round2_complete = NODE_IDS.iter().all(|nid| {
        nodes
            .get(*nid)
            .map(|ns| ns.round2 == "complete")
            .unwrap_or(false)
    });
    let any_round2_complete = NODE_IDS.iter().any(|nid| {
        nodes
            .get(*nid)
            .map(|ns| ns.round2 == "complete")
            .unwrap_or(false)
    });

    if all_round2_complete {
        "round2_in_progress".to_string() // Ready for aggregation
    } else if any_round2_complete || all_round1_complete {
        "round2_in_progress".to_string()
    } else if any_round1_complete {
        "round1_in_progress".to_string()
    } else {
        "pending".to_string()
    }
}

/// Execute Signing Round 1 for a specific node.
///
/// Forwards a request to the node's `/api/signing/round1` endpoint
/// with the signing request ID and wallet index.
async fn execute_signing_round1(
    state: &AppState,
    signing_request: &crate::models::signing::SigningRequestRow,
    node_id: &str,
    node_url: &str,
) -> AppResult<()> {
    // Precondition: signing request must be pending or round1_in_progress
    match signing_request.status.as_str() {
        "pending" | "round1_in_progress" => {}
        _ => {
            return Err(AppError::RoundPrecondition {
                message: format!(
                    "Round 1 requires status pending or round1_in_progress, got {}",
                    signing_request.status
                ),
            });
        }
    }

    let url = format!("{node_url}/api/signing/round1");
    let body = serde_json::json!({
        "signing_request_id": signing_request.id,
        "wallet_index": signing_request.wallet_index,
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
        message: format!("Failed to parse round1 response: {e}"),
    })?;

    // Extract commitments from the response
    let commitments = resp.get("commitments").cloned().ok_or_else(|| {
        AppError::NodeError {
            node_id: node_id.to_string(),
            message: "commitments missing from round1 response".to_string(),
        }
    })?;

    // Store commitments as output_data for this round
    db::signing::complete_signing_round(
        &state.pool,
        signing_request.id,
        node_id,
        1,
        Some(commitments),
    )
    .await?;

    tracing::info!(
        node_id,
        signing_request_id = %signing_request.id,
        "Signing Round 1 complete for node"
    );

    Ok(())
}

/// Execute Signing Round 2 for a specific node.
///
/// Precondition: both nodes must have completed Round 1.
/// Builds the transaction message, forwards commitments and message
/// to the node's `/api/signing/round2` endpoint.
async fn execute_signing_round2(
    state: &AppState,
    signing_request: &crate::models::signing::SigningRequestRow,
    node_id: &str,
    node_url: &str,
) -> AppResult<()> {
    // Precondition: status must allow round 2
    match signing_request.status.as_str() {
        "round1_in_progress" | "round2_in_progress" => {}
        _ => {
            return Err(AppError::RoundPrecondition {
                message: format!(
                    "Round 2 requires status round1_in_progress or round2_in_progress, got {}",
                    signing_request.status
                ),
            });
        }
    }

    let _other_id = other_node_id(node_id);

    // Precondition: both nodes must have completed Round 1
    let round_states = db::signing::get_signing_round_states(
        &state.pool,
        signing_request.id,
    )
    .await?;

    let mut commitments_map: HashMap<String, serde_json::Value> = HashMap::new();

    for nid in &NODE_IDS {
        let r1 = round_states
            .iter()
            .find(|rs| rs.node_id == *nid && rs.round == 1)
            .ok_or_else(|| AppError::Internal {
                message: format!("Round 1 state for {nid} not found"),
            })?;

        if r1.status != "complete" {
            return Err(AppError::RoundPrecondition {
                message: format!(
                    "{nid} has not completed Round 1"
                ),
            });
        }

        let commitment_data = r1.output_data.clone().ok_or_else(|| AppError::Internal {
            message: format!("{nid} Round 1 output_data is null"),
        })?;

        commitments_map.insert(nid.to_string(), commitment_data);
    }

    // Build or retrieve the transaction message.
    // We build it once and store it so all nodes sign the same message.
    let message_bytes = if let Some(ref stored_msg) = signing_request.tx_message {
        stored_msg.clone()
    } else {
        let msg_bytes = build_solana_message(state, signing_request).await?;
        db::signing::update_signing_request_tx_message(
            &state.pool,
            signing_request.id,
            &msg_bytes,
        )
        .await?;
        msg_bytes
    };

    // Base64-encode the message for the node
    use base64::Engine;
    let message_b64 = base64::engine::general_purpose::STANDARD.encode(&message_bytes);

    let url = format!("{node_url}/api/signing/round2");
    let body = serde_json::json!({
        "signing_request_id": signing_request.id,
        "wallet_index": signing_request.wallet_index,
        "message": message_b64,
        "commitments": commitments_map,
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

    // Extract signature share
    let signature_share = resp.get("signature_share").cloned().ok_or_else(|| {
        AppError::NodeError {
            node_id: node_id.to_string(),
            message: "signature_share missing from round2 response".to_string(),
        }
    })?;

    // Store signature share as output_data for round 2
    db::signing::complete_signing_round(
        &state.pool,
        signing_request.id,
        node_id,
        2,
        Some(signature_share),
    )
    .await?;

    tracing::info!(
        node_id,
        signing_request_id = %signing_request.id,
        "Signing Round 2 complete for node"
    );

    Ok(())
}

/// Build the Solana transaction message bytes for signing.
///
/// Creates a SOL transfer instruction and serializes the message.
async fn build_solana_message(
    state: &AppState,
    signing_request: &crate::models::signing::SigningRequestRow,
) -> AppResult<Vec<u8>> {
    // Look up the sender wallet
    let wallet =
        db::wallet::get_wallet_by_index(&state.pool, signing_request.wallet_index)
            .await?
            .ok_or(AppError::WalletNotFound {
                index: signing_request.wallet_index,
            })?;

    // Parse sender pubkey
    let from_pubkey_bytes = bs58::decode(&wallet.address)
        .into_vec()
        .map_err(|e| AppError::Internal {
            message: format!("Failed to decode sender address: {e}"),
        })?;
    let from_pubkey =
        Pubkey::try_from(from_pubkey_bytes.as_slice()).map_err(|e| AppError::Internal {
            message: format!("Invalid sender pubkey: {e}"),
        })?;

    // Parse recipient pubkey
    let to_pubkey_bytes = bs58::decode(&signing_request.recipient)
        .into_vec()
        .map_err(|e| AppError::Internal {
            message: format!("Failed to decode recipient address: {e}"),
        })?;
    let to_pubkey =
        Pubkey::try_from(to_pubkey_bytes.as_slice()).map_err(|e| AppError::Internal {
            message: format!("Invalid recipient pubkey: {e}"),
        })?;

    // Get recent blockhash from Solana RPC
    let rpc_client =
        solana_client::rpc_client::RpcClient::new(&state.config.solana_rpc_url);
    let blockhash = rpc_client
        .get_latest_blockhash()
        .map_err(|e| AppError::SolanaRpcError {
            message: format!("Failed to get latest blockhash: {e}"),
        })?;

    // Create transfer instruction using the system program
    let instruction = system_transfer(
        &from_pubkey,
        &to_pubkey,
        signing_request.amount_lamports as u64,
    );

    // Build the message
    let message =
        solana_sdk::message::Message::new_with_blockhash(&[instruction], Some(&from_pubkey), &blockhash);

    // Serialize the message -- this is what gets signed
    let message_bytes = message.serialize();

    Ok(message_bytes)
}

/// `POST /api/signing-requests/{id}/aggregate` -- aggregate signatures and broadcast.
async fn aggregate(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> AppResult<Json<AggregateResponse>> {
    // Load signing request
    let signing_request = db::signing::get_signing_request(&state.pool, id)
        .await?
        .ok_or(AppError::SigningRequestNotFound { id })?;

    // Must be in round2_in_progress status
    if signing_request.status != "round2_in_progress" {
        return Err(AppError::InvalidStatus);
    }

    // Verify both nodes completed both rounds
    let round_states =
        db::signing::get_signing_round_states(&state.pool, id).await?;

    for nid in &NODE_IDS {
        for round in 1..=2i16 {
            let rs = round_states
                .iter()
                .find(|rs| rs.node_id == *nid && rs.round == round)
                .ok_or_else(|| AppError::Internal {
                    message: format!("Round state not found for {nid} round {round}"),
                })?;
            if rs.status != "complete" {
                return Err(AppError::RoundPrecondition {
                    message: format!(
                        "{nid} has not completed round {round}"
                    ),
                });
            }
        }
    }

    // Update status to aggregating
    db::signing::update_signing_request_status(&state.pool, id, "aggregating").await?;

    // Retrieve the stored transaction message
    let tx_message_bytes = signing_request.tx_message.clone().ok_or_else(|| {
        AppError::Internal {
            message: "Transaction message not set (Round 2 was not completed properly)".to_string(),
        }
    })?;

    // Build the commitments map for the SigningPackage
    let mut commitments_map: BTreeMap<Identifier, frost::round1::SigningCommitments> =
        BTreeMap::new();

    for nid in &NODE_IDS {
        let r1 = round_states
            .iter()
            .find(|rs| rs.node_id == *nid && rs.round == 1)
            .unwrap();
        let commitment_json = r1.output_data.as_ref().ok_or_else(|| AppError::Internal {
            message: format!("{nid} Round 1 output_data is null"),
        })?;

        let identifier = node_id_to_identifier(nid)?;
        let commitment: frost::round1::SigningCommitments =
            serde_json::from_value(commitment_json.clone()).map_err(|e| {
                AppError::AggregationFailed {
                    message: format!("Failed to deserialize commitments from {nid}: {e}"),
                }
            })?;
        commitments_map.insert(identifier, commitment);
    }

    // Build the signature shares map
    let mut signature_shares_map: BTreeMap<Identifier, frost::round2::SignatureShare> =
        BTreeMap::new();

    for nid in &NODE_IDS {
        let r2 = round_states
            .iter()
            .find(|rs| rs.node_id == *nid && rs.round == 2)
            .unwrap();
        let share_json = r2.output_data.as_ref().ok_or_else(|| AppError::Internal {
            message: format!("{nid} Round 2 output_data is null"),
        })?;

        let identifier = node_id_to_identifier(nid)?;
        let share: frost::round2::SignatureShare =
            serde_json::from_value(share_json.clone()).map_err(|e| {
                AppError::AggregationFailed {
                    message: format!(
                        "Failed to deserialize signature share from {nid}: {e}"
                    ),
                }
            })?;
        signature_shares_map.insert(identifier, share);
    }

    // Build the SigningPackage (same commitments + message as used in round 2)
    let signing_package =
        frost::SigningPackage::new(commitments_map, &tx_message_bytes);

    // Derive the child public key package for this wallet index
    let session = db::dkg::get_completed_session(&state.pool)
        .await?
        .ok_or(AppError::DkgNotComplete)?;

    let group_public_key = session.group_public_key.ok_or_else(|| AppError::Internal {
        message: "DKG complete but group_public_key is null".to_string(),
    })?;

    // Gather verifying shares from DKG round 3 output
    let dkg_round_states = db::dkg::get_round_states(&state.pool, session.id).await?;
    let mut verifying_shares_b58: BTreeMap<String, String> = BTreeMap::new();

    for nid in &NODE_IDS {
        let r3 = dkg_round_states
            .iter()
            .find(|rs| rs.node_id == *nid && rs.round == 3 && rs.status == "complete")
            .ok_or_else(|| AppError::Internal {
                message: format!("DKG Round 3 not complete for {nid}"),
            })?;
        let output = r3.output_package.as_ref().ok_or_else(|| AppError::Internal {
            message: format!("{nid} DKG Round 3 output_package is null"),
        })?;
        let vs = output
            .get("verifying_share")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Internal {
                message: format!("verifying_share not found in {nid} DKG Round 3 output"),
            })?
            .to_string();
        verifying_shares_b58.insert(nid.to_string(), vs);
    }

    let child_public_key_package = derivation::derive_child_public_key_package(
        &group_public_key,
        &verifying_shares_b58,
        signing_request.wallet_index as u32,
    )?;

    // Aggregate signature shares
    let group_signature = frost::aggregate(
        &signing_package,
        &signature_shares_map,
        &child_public_key_package,
    )
    .map_err(|e| AppError::AggregationFailed {
        message: format!("frost::aggregate failed: {e}"),
    })?;

    tracing::info!(
        signing_request_id = %id,
        "FROST signature aggregated successfully"
    );

    // Build and broadcast the Solana transaction
    match broadcast_transaction(&state, &signing_request, &tx_message_bytes, &group_signature)
        .await
    {
        Ok(tx_sig) => {
            let tx_sig_str = tx_sig.to_string();
            let explorer_url = format!(
                "https://explorer.solana.com/tx/{tx_sig_str}?cluster=devnet"
            );

            // Update status to broadcasted
            db::signing::update_signing_request_tx(
                &state.pool,
                id,
                "broadcasted",
                Some(&tx_sig_str),
                None,
            )
            .await?;

            tracing::info!(
                signing_request_id = %id,
                tx_signature = %tx_sig_str,
                "Transaction broadcasted to Solana Devnet"
            );

            // Spawn async confirmation polling
            let pool = state.pool.clone();
            let rpc_url = state.config.solana_rpc_url.clone();
            let tx_sig_for_poll = tx_sig_str.clone();
            tokio::spawn(async move {
                poll_confirmation(pool, id, &tx_sig_for_poll, &rpc_url).await;
            });

            Ok(Json(AggregateResponse {
                signing_request_id: id,
                status: "broadcasted".to_string(),
                tx_signature: Some(tx_sig_str),
                explorer_url: Some(explorer_url),
            }))
        }
        Err(e) => {
            let err_msg = format!("{e}");
            db::signing::update_signing_request_tx(
                &state.pool,
                id,
                "failed",
                None,
                Some(&err_msg),
            )
            .await?;

            Err(AppError::BroadcastFailed { message: err_msg })
        }
    }
}

/// Build a full Solana transaction with the FROST signature and broadcast it.
async fn broadcast_transaction(
    state: &AppState,
    _signing_request: &crate::models::signing::SigningRequestRow,
    tx_message_bytes: &[u8],
    group_signature: &frost::Signature,
) -> AppResult<SolSignature> {
    // Deserialize the stored message using bincode (same format as Message::serialize)
    let message: solana_sdk::message::Message =
        bincode::deserialize(tx_message_bytes).map_err(|e| AppError::Internal {
            message: format!("Failed to deserialize stored tx message: {e}"),
        })?;

    // Extract the FROST group signature bytes.
    // frost-ed25519 Signature serializes as 64 bytes (R || s).
    let frost_sig_bytes = group_signature.serialize().map_err(|e| AppError::Internal {
        message: format!("Failed to serialize FROST signature: {e}"),
    })?;

    // Create a Solana Signature from the 64 bytes
    let sol_sig = SolSignature::try_from(frost_sig_bytes.as_slice()).map_err(|e| {
        AppError::Internal {
            message: format!("Failed to convert FROST signature to Solana signature: {e}"),
        }
    })?;

    // Build the transaction with the pre-signed signature.
    // A Solana Transaction consists of signatures + message.
    // For a single-signer transfer, there is exactly one signature.
    let mut tx = Transaction::new_unsigned(message);
    tx.signatures = vec![sol_sig];

    // Broadcast via RPC
    let rpc_client =
        solana_client::rpc_client::RpcClient::new(&state.config.solana_rpc_url);

    let tx_signature = rpc_client
        .send_transaction(&tx)
        .map_err(|e| AppError::BroadcastFailed {
            message: format!("send_transaction failed: {e}"),
        })?;

    Ok(tx_signature)
}

/// Poll Solana for transaction confirmation and update status.
///
/// Runs in a background task after broadcast. Checks up to 30 times
/// with 2-second intervals.
async fn poll_confirmation(
    pool: sqlx::PgPool,
    signing_request_id: uuid::Uuid,
    tx_signature: &str,
    rpc_url: &str,
) {
    let rpc_client = solana_client::rpc_client::RpcClient::new(rpc_url);
    let sig = match tx_signature.parse::<SolSignature>() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(
                %signing_request_id,
                "Failed to parse tx signature for confirmation polling: {e}"
            );
            return;
        }
    };

    for attempt in 1..=30 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        match rpc_client.get_signature_statuses(&[sig]) {
            Ok(response) => {
                if let Some(Some(status)) = response.value.first() {
                    // 1. Check for on-chain error first
                    if status.err.is_some() {
                        let err_msg = format!(
                            "Transaction failed on-chain: {:?}",
                            status.err
                        );
                        let _ = db::signing::update_signing_request_tx(
                            &pool,
                            signing_request_id,
                            "failed",
                            Some(tx_signature),
                            Some(&err_msg),
                        )
                        .await;
                        tracing::error!(
                            %signing_request_id,
                            "Transaction failed on Solana: {err_msg}"
                        );
                        return;
                    }

                    // 2. Check confirmation level — only accept confirmed or finalized,
                    //    not merely processed (which could still be dropped).
                    let is_confirmed = status.confirmation_status.as_ref().is_some_and(|cs| {
                        use solana_transaction_status::TransactionConfirmationStatus;
                        matches!(
                            cs,
                            TransactionConfirmationStatus::Confirmed
                                | TransactionConfirmationStatus::Finalized
                        )
                    });

                    if is_confirmed {
                        if let Err(e) = db::signing::update_signing_request_tx(
                            &pool,
                            signing_request_id,
                            "confirmed",
                            Some(tx_signature),
                            None,
                        )
                        .await
                        {
                            tracing::error!(
                                %signing_request_id,
                                "Failed to update status to confirmed: {e}"
                            );
                        } else {
                            tracing::info!(
                                %signing_request_id,
                                attempt,
                                "Transaction confirmed on Solana"
                            );
                        }
                        return;
                    }

                    // 3. Still at processed level — continue polling
                }
            }
            Err(e) => {
                tracing::warn!(
                    %signing_request_id,
                    attempt,
                    "Failed to poll signature status: {e}"
                );
            }
        }
    }

    // Timed out waiting for confirmation
    let _ = db::signing::update_signing_request_tx(
        &pool,
        signing_request_id,
        "failed",
        Some(tx_signature),
        Some("Transaction confirmation timed out after 60 seconds"),
    )
    .await;
    tracing::warn!(
        %signing_request_id,
        "Transaction confirmation timed out"
    );
}
