use std::collections::BTreeMap;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use frost_ed25519::keys::{KeyPackage, PublicKeyPackage};
use frost_ed25519::{round1, round2, Identifier, SigningPackage};

use crate::db;
use crate::derivation;
use crate::error::{AppError, AppResult};
use crate::models::signing::{
    SigningRound1Request, SigningRound1Response, SigningRound2Request, SigningRound2Response,
};
use crate::AppState;

/// Build the signing sub-router for the TSS Node.
///
/// Mounted at `/api/signing` in the top-level router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/round1", post(round1_handler))
        .route("/round2", post(round2_handler))
}

/// Map a node ID string (e.g. "node-a") to a FROST Identifier.
fn node_id_to_identifier(node_id: &str) -> Result<Identifier, AppError> {
    Identifier::derive(node_id.as_bytes()).map_err(|e| AppError::CryptoError {
        message: format!("Failed to derive identifier for {node_id}: {e}"),
    })
}

/// `POST /api/signing/round1` -- execute Signing Round 1.
///
/// Derives the child key share for the wallet index, then generates
/// nonce commitments using `frost::round1::commit`.
async fn round1_handler(
    State(state): State<AppState>,
    Json(body): Json<SigningRound1Request>,
) -> AppResult<Json<SigningRound1Response>> {
    let node_id = &state.config.node_id;
    let signing_request_id = body.signing_request_id;
    let wallet_index = body.wallet_index;

    tracing::info!(
        node_id,
        %signing_request_id,
        wallet_index,
        "Signing Round 1 starting"
    );

    // Check if nonces already exist for this signing request (round already done)
    let existing = db::signing::get_nonces(&state.pool, signing_request_id).await?;
    if existing.is_some() {
        return Err(AppError::RoundAlreadyDone { round: 1 });
    }

    // Get the root key share from DKG
    let key_share = db::dkg::get_latest_key_share(&state.pool)
        .await?
        .ok_or(AppError::DkgNotComplete)?;

    // Deserialize root KeyPackage and PublicKeyPackage
    let root_key_package: KeyPackage =
        serde_json::from_value(key_share.key_package).map_err(|e| AppError::Internal {
            message: format!("Failed to deserialize root key_package: {e}"),
        })?;
    let root_public_key_package: PublicKeyPackage =
        serde_json::from_value(key_share.public_key_package).map_err(|e| AppError::Internal {
            message: format!("Failed to deserialize root public_key_package: {e}"),
        })?;

    // Derive child key package for the given wallet index
    let (child_key_package, _child_public_key_package) = derivation::derive_child_key_package(
        &root_key_package,
        &root_public_key_package,
        wallet_index as u32,
    )?;

    // Generate nonces and commitments using the child signing share
    let mut rng = rand::rngs::OsRng;
    let (nonces, commitments) = round1::commit(child_key_package.signing_share(), &mut rng);

    // Serialize nonces for storage (single-use, will be consumed in round 2)
    let nonces_json = serde_json::to_value(&nonces).map_err(|e| AppError::Internal {
        message: format!("Failed to serialize nonces: {e}"),
    })?;

    // Persist nonces locally (never sent to coordinator)
    db::signing::insert_nonces(&state.pool, signing_request_id, &nonces_json).await?;

    // Serialize commitments for the response
    let commitments_json = serde_json::to_value(&commitments).map_err(|e| AppError::Internal {
        message: format!("Failed to serialize commitments: {e}"),
    })?;

    tracing::info!(
        node_id,
        %signing_request_id,
        wallet_index,
        "Signing Round 1 complete"
    );

    Ok(Json(SigningRound1Response {
        node_id: node_id.clone(),
        signing_request_id,
        commitments: commitments_json,
    }))
}

/// `POST /api/signing/round2` -- execute Signing Round 2.
///
/// Uses stored nonces, derived child key share, all nodes' commitments,
/// and the transaction message to produce a signature share.
async fn round2_handler(
    State(state): State<AppState>,
    Json(body): Json<SigningRound2Request>,
) -> AppResult<Json<SigningRound2Response>> {
    let node_id = &state.config.node_id;
    let signing_request_id = body.signing_request_id;
    let wallet_index = body.wallet_index;

    tracing::info!(
        node_id,
        %signing_request_id,
        wallet_index,
        "Signing Round 2 starting"
    );

    // Retrieve stored nonces (must exist from round 1)
    let nonces_row = db::signing::get_nonces(&state.pool, signing_request_id)
        .await?
        .ok_or_else(|| AppError::RoundPrecondition {
            message: "Signing Round 1 not completed for this node (no nonces found)".to_string(),
        })?;

    // Deserialize nonces
    let nonces: round1::SigningNonces =
        serde_json::from_value(nonces_row.nonces).map_err(|e| AppError::Internal {
            message: format!("Failed to deserialize stored nonces: {e}"),
        })?;

    // Get the root key share from DKG
    let key_share = db::dkg::get_latest_key_share(&state.pool)
        .await?
        .ok_or(AppError::DkgNotComplete)?;

    // Deserialize root KeyPackage and PublicKeyPackage
    let root_key_package: KeyPackage =
        serde_json::from_value(key_share.key_package).map_err(|e| AppError::Internal {
            message: format!("Failed to deserialize root key_package: {e}"),
        })?;
    let root_public_key_package: PublicKeyPackage =
        serde_json::from_value(key_share.public_key_package).map_err(|e| AppError::Internal {
            message: format!("Failed to deserialize root public_key_package: {e}"),
        })?;

    // Derive child key package
    let (child_key_package, _child_public_key_package) = derivation::derive_child_key_package(
        &root_key_package,
        &root_public_key_package,
        wallet_index as u32,
    )?;

    // Decode the transaction message from Base64
    use base64::Engine;
    let message_bytes = base64::engine::general_purpose::STANDARD
        .decode(&body.message)
        .map_err(|e| AppError::InvalidMessage {
            message: format!("Invalid base64 message: {e}"),
        })?;

    // Build the commitments map: Identifier -> SigningCommitments
    let mut commitments_map: BTreeMap<Identifier, round1::SigningCommitments> = BTreeMap::new();

    for (nid, commitment_json) in &body.commitments {
        let identifier = node_id_to_identifier(nid)?;
        let commitment: round1::SigningCommitments =
            serde_json::from_value(commitment_json.clone()).map_err(|e| {
                AppError::InvalidCommitments {
                    message: format!("Failed to deserialize commitments from {nid}: {e}"),
                }
            })?;
        commitments_map.insert(identifier, commitment);
    }

    // Create the signing package
    let signing_package = SigningPackage::new(commitments_map, &message_bytes);

    // Compute signature share
    let signature_share =
        round2::sign(&signing_package, &nonces, &child_key_package).map_err(|e| {
            AppError::CryptoError {
                message: format!("round2::sign failed: {e}"),
            }
        })?;

    // Delete consumed nonces (nonce reuse would compromise the private key)
    db::signing::delete_nonces(&state.pool, signing_request_id).await?;

    // Serialize signature share for the response
    let signature_share_json =
        serde_json::to_value(&signature_share).map_err(|e| AppError::Internal {
            message: format!("Failed to serialize signature share: {e}"),
        })?;

    tracing::info!(
        node_id,
        %signing_request_id,
        wallet_index,
        "Signing Round 2 complete -- nonces consumed and deleted"
    );

    Ok(Json(SigningRound2Response {
        node_id: node_id.clone(),
        signing_request_id,
        signature_share: signature_share_json,
    }))
}
