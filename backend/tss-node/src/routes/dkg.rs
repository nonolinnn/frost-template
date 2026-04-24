use std::collections::BTreeMap;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use frost_ed25519::keys::dkg;
use frost_ed25519::Identifier;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::dkg::{
    DkgRound1Request, DkgRound1Response, DkgRound2Request, DkgRound2Response, DkgRound3Request,
    DkgRound3Response,
};
use crate::AppState;

/// Build the DKG sub-router for the TSS Node.
///
/// Mounted at `/api/dkg` in the top-level router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/round1", post(round1))
        .route("/round2", post(round2))
        .route("/round3", post(round3))
}

/// Map a node ID string (e.g. "node-a") to a FROST Identifier by deriving
/// it from the node ID bytes. This ensures deterministic, consistent
/// identifiers across services.
fn node_id_to_identifier(node_id: &str) -> Result<Identifier, AppError> {
    Identifier::derive(node_id.as_bytes()).map_err(|e| AppError::CryptoError {
        message: format!("Failed to derive identifier for {node_id}: {e}"),
    })
}

/// Get the "other" node ID given this node's ID.
fn other_node_id(my_node_id: &str) -> &'static str {
    if my_node_id == "node-a" {
        "node-b"
    } else {
        "node-a"
    }
}

/// `POST /api/dkg/round1` -- execute DKG Round 1.
///
/// Generates the node's commitment package using `dkg::part1`.
/// Persists the secret package in the node's DB and returns the public package.
async fn round1(
    State(state): State<AppState>,
    Json(body): Json<DkgRound1Request>,
) -> AppResult<Json<DkgRound1Response>> {
    let node_id = &state.config.node_id;
    let session_id = body.session_id;

    tracing::info!(node_id, %session_id, "DKG Round 1 starting");

    // Check if this round was already completed
    let existing = db::dkg::get_round_data(&state.pool, session_id, 1).await?;
    if existing.is_some() {
        return Err(AppError::RoundAlreadyDone { round: 1 });
    }

    // Derive FROST identifier for this node
    let identifier = node_id_to_identifier(node_id)?;

    // Execute DKG part1: generates secret polynomial and commitment package
    let mut rng = rand::rngs::OsRng;
    let (round1_secret, round1_package) =
        dkg::part1(identifier, 2, 2, &mut rng).map_err(|e| AppError::CryptoError {
            message: format!("dkg::part1 failed: {e}"),
        })?;

    // Serialize for storage
    let secret_json = serde_json::to_value(&round1_secret).map_err(|e| AppError::Internal {
        message: format!("Failed to serialize round1 secret: {e}"),
    })?;
    let package_json =
        serde_json::to_value(&round1_package).map_err(|e| AppError::Internal {
            message: format!("Failed to serialize round1 package: {e}"),
        })?;

    // Persist secret locally (never leaves the node)
    db::dkg::insert_round_data(&state.pool, session_id, 1, &secret_json).await?;

    tracing::info!(node_id, %session_id, "DKG Round 1 complete");

    Ok(Json(DkgRound1Response {
        node_id: node_id.clone(),
        session_id,
        round1_package: package_json,
    }))
}

/// `POST /api/dkg/round2` -- execute DKG Round 2.
///
/// Receives the other node's Round 1 package, uses this node's Round 1 secret
/// to compute Round 2 packages via `dkg::part2`.
async fn round2(
    State(state): State<AppState>,
    Json(body): Json<DkgRound2Request>,
) -> AppResult<Json<DkgRound2Response>> {
    let node_id = &state.config.node_id;
    let session_id = body.session_id;

    tracing::info!(node_id, %session_id, "DKG Round 2 starting");

    // Check precondition: Round 1 must be completed
    let round1_data = db::dkg::get_round_data(&state.pool, session_id, 1)
        .await?
        .ok_or_else(|| AppError::RoundPrecondition {
            message: "Round 1 not completed for this node".to_string(),
        })?;

    // Check if Round 2 was already completed
    let existing = db::dkg::get_round_data(&state.pool, session_id, 2).await?;
    if existing.is_some() {
        return Err(AppError::RoundAlreadyDone { round: 2 });
    }

    // Deserialize our Round 1 secret
    let round1_secret: dkg::round1::SecretPackage =
        serde_json::from_value(round1_data.secret_package).map_err(|e| AppError::Internal {
            message: format!("Failed to deserialize round1 secret: {e}"),
        })?;

    // Build the BTreeMap<Identifier, round1::Package> from the other node's package
    let other_id = other_node_id(node_id);
    let other_package_json = body
        .round1_packages
        .get(other_id)
        .ok_or_else(|| AppError::InvalidPackages {
            message: format!("Missing round1 package from {other_id}"),
        })?;

    let other_identifier = node_id_to_identifier(other_id)?;
    let other_package: dkg::round1::Package =
        serde_json::from_value(other_package_json.clone()).map_err(|e| {
            AppError::InvalidPackages {
                message: format!("Failed to deserialize round1 package from {other_id}: {e}"),
            }
        })?;

    let mut round1_packages = BTreeMap::new();
    round1_packages.insert(other_identifier, other_package);

    // Execute DKG part2
    let (round2_secret, round2_packages) =
        dkg::part2(round1_secret, &round1_packages).map_err(|e| AppError::CryptoError {
            message: format!("dkg::part2 failed: {e}"),
        })?;

    // The round2_packages map contains one entry: our package for the other node
    let round2_package_for_other = round2_packages
        .get(&other_identifier)
        .ok_or_else(|| AppError::Internal {
            message: "dkg::part2 did not produce a package for the other node".to_string(),
        })?;

    // Serialize for storage
    let secret_json = serde_json::to_value(&round2_secret).map_err(|e| AppError::Internal {
        message: format!("Failed to serialize round2 secret: {e}"),
    })?;
    let package_json = serde_json::to_value(round2_package_for_other).map_err(|e| {
        AppError::Internal {
            message: format!("Failed to serialize round2 package: {e}"),
        }
    })?;

    // Persist Round 2 secret locally
    db::dkg::insert_round_data(&state.pool, session_id, 2, &secret_json).await?;

    tracing::info!(node_id, %session_id, "DKG Round 2 complete");

    Ok(Json(DkgRound2Response {
        node_id: node_id.clone(),
        session_id,
        round2_package: package_json,
    }))
}

/// `POST /api/dkg/round3` -- execute DKG Round 3 (finalize).
///
/// Receives Round 1 and Round 2 packages from the other node, uses this
/// node's Round 2 secret to compute the final key share and group verifying key
/// via `dkg::part3`. Persists the KeyPackage in the node's DB.
async fn round3(
    State(state): State<AppState>,
    Json(body): Json<DkgRound3Request>,
) -> AppResult<Json<DkgRound3Response>> {
    let node_id = &state.config.node_id;
    let session_id = body.session_id;

    tracing::info!(node_id, %session_id, "DKG Round 3 starting");

    // Check precondition: Round 2 must be completed
    let round2_data = db::dkg::get_round_data(&state.pool, session_id, 2)
        .await?
        .ok_or_else(|| AppError::RoundPrecondition {
            message: "Round 2 not completed for this node".to_string(),
        })?;

    // Check if already completed (key share exists)
    let existing_key = db::dkg::get_key_share_by_session(&state.pool, session_id).await?;
    if existing_key.is_some() {
        return Err(AppError::RoundAlreadyDone { round: 3 });
    }

    // Deserialize our Round 2 secret
    let round2_secret: dkg::round2::SecretPackage =
        serde_json::from_value(round2_data.secret_package).map_err(|e| AppError::Internal {
            message: format!("Failed to deserialize round2 secret: {e}"),
        })?;

    let other_id = other_node_id(node_id);
    let other_identifier = node_id_to_identifier(other_id)?;

    // Parse the other node's Round 1 package
    let other_r1_json =
        body.round1_packages
            .get(other_id)
            .ok_or_else(|| AppError::InvalidPackages {
                message: format!("Missing round1 package from {other_id}"),
            })?;
    let other_r1_package: dkg::round1::Package =
        serde_json::from_value(other_r1_json.clone()).map_err(|e| AppError::InvalidPackages {
            message: format!("Failed to deserialize round1 package from {other_id}: {e}"),
        })?;

    let mut round1_packages = BTreeMap::new();
    round1_packages.insert(other_identifier, other_r1_package);

    // Parse the other node's Round 2 package
    let other_r2_json =
        body.round2_packages
            .get(other_id)
            .ok_or_else(|| AppError::InvalidPackages {
                message: format!("Missing round2 package from {other_id}"),
            })?;
    let other_r2_package: dkg::round2::Package =
        serde_json::from_value(other_r2_json.clone()).map_err(|e| AppError::InvalidPackages {
            message: format!("Failed to deserialize round2 package from {other_id}: {e}"),
        })?;

    let mut round2_packages = BTreeMap::new();
    round2_packages.insert(other_identifier, other_r2_package);

    // Execute DKG part3 (finalize)
    let (key_package, public_key_package) =
        dkg::part3(&round2_secret, &round1_packages, &round2_packages).map_err(|e| {
            AppError::CryptoError {
                message: format!("dkg::part3 failed: {e}"),
            }
        })?;

    // Extract the group verifying key and encode as Base58
    let verifying_key = public_key_package.verifying_key();
    let vk_bytes = verifying_key.serialize().map_err(|e| AppError::CryptoError {
        message: format!("Failed to serialize verifying key: {e}"),
    })?;
    let group_public_key = bs58::encode(&vk_bytes).into_string();

    // Extract this node's verifying share and encode as Base58
    let my_identifier = node_id_to_identifier(node_id)?;
    let verifying_share = public_key_package
        .verifying_shares()
        .get(&my_identifier)
        .ok_or_else(|| AppError::Internal {
            message: "Verifying share not found for this node".to_string(),
        })?;
    let vs_bytes = verifying_share.serialize().map_err(|e| AppError::CryptoError {
        message: format!("Failed to serialize verifying share: {e}"),
    })?;
    let verifying_share_b58 = bs58::encode(&vs_bytes).into_string();

    // Serialize and persist key material
    let key_package_json =
        serde_json::to_value(&key_package).map_err(|e| AppError::Internal {
            message: format!("Failed to serialize key_package: {e}"),
        })?;
    let public_key_package_json =
        serde_json::to_value(&public_key_package).map_err(|e| AppError::Internal {
            message: format!("Failed to serialize public_key_package: {e}"),
        })?;

    db::dkg::insert_key_share(
        &state.pool,
        session_id,
        &key_package_json,
        &public_key_package_json,
        &group_public_key,
    )
    .await?;

    tracing::info!(
        node_id,
        %session_id,
        %group_public_key,
        "DKG Round 3 complete — key share persisted"
    );

    Ok(Json(DkgRound3Response {
        node_id: node_id.clone(),
        session_id,
        group_public_key,
        verifying_share: verifying_share_b58,
    }))
}
