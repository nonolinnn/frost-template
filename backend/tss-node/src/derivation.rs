//! HD wallet derivation for TSS Node.
//!
//! Derives child key shares from the root FROST key share using hd-wallet
//! Edwards non-hardened derivation. The shift produced by derivation is
//! added to the node's signing share, producing a child signing share that
//! is compatible with the child public key derived by the Coordinator.
//!
//! **Security property**: non-hardened derivation preserves the additive
//! threshold relationship. If the root key is shared as root = s_a + s_b,
//! then child = root + shift = (s_a + shift) + (s_b + shift) - shift
//! ... which doesn't work directly. Instead, each node adds the shift
//! independently, and the Lagrange coefficients handle the rest because
//! the shift is a public value added to each share:
//! child = sum(lagrange_i * (share_i + shift)) = sum(lagrange_i * share_i) + shift * sum(lagrange_i)
//! = root + shift * 1 = root + shift (since sum of Lagrange coefficients = 1).

use std::collections::BTreeMap;

use frost_ed25519::keys::{KeyPackage, PublicKeyPackage, SigningShare, VerifyingShare};
use frost_ed25519::{Identifier, VerifyingKey};
use generic_ec::curves::Ed25519;
use generic_ec::{Point, Scalar, SecretScalar};
use hd_wallet::{Edwards, ExtendedPublicKey, HdWallet, NonHardenedIndex};
use sha2::{Digest, Sha256};

use crate::error::AppError;

/// Deterministic chain code derived from the group public key.
/// Must match the coordinator's derivation exactly.
fn derive_chain_code(group_vk_bytes: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"frost-hd-wallet-chain-code");
    hasher.update(group_vk_bytes);
    hasher.finalize().into()
}

/// Convert FROST VerifyingKey bytes (32-byte compressed Edwards Y) to
/// an hd-wallet ExtendedPublicKey.
fn frost_vk_bytes_to_extended_pk(
    vk_bytes: &[u8; 32],
) -> Result<ExtendedPublicKey<Ed25519>, AppError> {
    let point = Point::<Ed25519>::from_bytes(vk_bytes).map_err(|e| AppError::Internal {
        message: format!("Failed to parse verifying key as Ed25519 point: {e}"),
    })?;

    let chain_code = derive_chain_code(vk_bytes);

    Ok(ExtendedPublicKey {
        public_key: point,
        chain_code,
    })
}

/// Derive the shift scalar for a given wallet index from the group public key.
///
/// This shift is the same value that both the coordinator and nodes compute.
/// The coordinator uses it implicitly (child_pk = parent_pk + shift*G).
/// The node uses it explicitly (child_share = parent_share + shift).
fn derive_shift(
    group_vk_bytes: &[u8; 32],
    wallet_index: u32,
) -> Result<Scalar<Ed25519>, AppError> {
    let parent_pk = frost_vk_bytes_to_extended_pk(group_vk_bytes)?;

    let child_index = NonHardenedIndex::try_from(wallet_index).map_err(|_| AppError::Internal {
        message: format!(
            "Wallet index {wallet_index} is out of range for non-hardened derivation"
        ),
    })?;

    let derived = <Edwards as hd_wallet::DeriveShift<Ed25519>>::derive_public_shift(
        &parent_pk,
        child_index,
    );

    Ok(derived.shift)
}

/// Derive a child KeyPackage for signing at a given wallet index.
///
/// The child signing share = parent signing share + shift.
/// The child verifying key = parent verifying key + shift * G (computed by derivation).
/// The child verifying share = parent verifying share + shift * G.
pub fn derive_child_key_package(
    key_package: &KeyPackage,
    public_key_package: &PublicKeyPackage,
    wallet_index: u32,
) -> Result<(KeyPackage, PublicKeyPackage), AppError> {
    // Get the group verifying key bytes
    let vk = public_key_package.verifying_key();
    let vk_bytes_vec = vk.serialize().map_err(|e| AppError::CryptoError {
        message: format!("Failed to serialize verifying key: {e}"),
    })?;
    let vk_bytes: [u8; 32] = vk_bytes_vec.try_into().map_err(|_| AppError::Internal {
        message: "Verifying key serialization is not 32 bytes".to_string(),
    })?;

    // Compute the derivation shift
    let shift = derive_shift(&vk_bytes, wallet_index)?;

    // Convert shift to curve25519-dalek scalar bytes.
    // generic-ec Scalar<Ed25519> serializes as big-endian bytes via to_be_bytes.
    // curve25519-dalek Scalar uses little-endian bytes.
    let shift_be_bytes = shift.to_be_bytes();
    let mut shift_le_bytes = [0u8; 32];
    for (i, b) in shift_be_bytes.as_ref().iter().enumerate() {
        shift_le_bytes[31 - i] = *b;
    }
    let dalek_shift =
        curve25519_dalek::Scalar::from_canonical_bytes(shift_le_bytes).into_option().ok_or_else(|| {
            AppError::CryptoError {
                message: "Derived shift is not a canonical scalar".to_string(),
            }
        })?;

    // Derive child signing share: parent_share + shift
    // We need to extract the raw scalar from the SigningShare, add the shift,
    // and create a new SigningShare. Since SigningShare::new() and to_scalar()
    // are pub(crate) in frost-core, we go through serde serialization.
    let parent_share_json = serde_json::to_value(key_package.signing_share())
        .map_err(|e| AppError::Internal {
            message: format!("Failed to serialize signing share: {e}"),
        })?;

    // SigningShare serializes as a hex string of the 32-byte scalar (little-endian)
    let parent_share_hex = parent_share_json.as_str().ok_or_else(|| AppError::Internal {
        message: "SigningShare JSON is not a string".to_string(),
    })?;
    let parent_share_bytes = hex::decode(parent_share_hex).map_err(|e| AppError::Internal {
        message: format!("Failed to decode signing share hex: {e}"),
    })?;

    let mut parent_scalar_bytes = [0u8; 32];
    parent_scalar_bytes.copy_from_slice(&parent_share_bytes);
    let parent_scalar =
        curve25519_dalek::Scalar::from_canonical_bytes(parent_scalar_bytes).into_option().ok_or_else(
            || AppError::CryptoError {
                message: "Parent signing share is not a canonical scalar".to_string(),
            },
        )?;

    // child_scalar = parent_scalar + shift
    let child_scalar = parent_scalar + dalek_shift;
    let child_scalar_hex = hex::encode(child_scalar.to_bytes());

    // Reconstruct child SigningShare via serde
    let child_share: SigningShare =
        serde_json::from_value(serde_json::Value::String(child_scalar_hex)).map_err(|e| {
            AppError::Internal {
                message: format!("Failed to deserialize child signing share: {e}"),
            }
        })?;

    // Derive child verifying key: use hd-wallet to get the proper child public key
    let parent_extended_pk = frost_vk_bytes_to_extended_pk(&vk_bytes)?;
    let child_index = NonHardenedIndex::try_from(wallet_index).map_err(|_| AppError::Internal {
        message: format!("Wallet index {wallet_index} out of range"),
    })?;
    let child_extended_pk = Edwards::derive_child_public_key(&parent_extended_pk, child_index);
    let child_vk_bytes = child_extended_pk.public_key.to_bytes(true);

    // Convert to FROST VerifyingKey via serde
    let child_vk_hex = hex::encode(child_vk_bytes.as_ref());
    let child_vk: VerifyingKey =
        serde_json::from_value(serde_json::Value::String(child_vk_hex)).map_err(|e| {
            AppError::Internal {
                message: format!("Failed to deserialize child verifying key: {e}"),
            }
        })?;

    // Derive child verifying shares for each participant.
    // child_verifying_share_i = parent_verifying_share_i + shift * G
    let shift_point = curve25519_dalek::constants::ED25519_BASEPOINT_POINT * dalek_shift;
    let mut child_verifying_shares = BTreeMap::new();

    for (id, parent_vs) in public_key_package.verifying_shares() {
        let parent_vs_json = serde_json::to_value(parent_vs).map_err(|e| AppError::Internal {
            message: format!("Failed to serialize verifying share: {e}"),
        })?;
        let parent_vs_hex =
            parent_vs_json
                .as_str()
                .ok_or_else(|| AppError::Internal {
                    message: "VerifyingShare JSON is not a string".to_string(),
                })?;
        let parent_vs_bytes = hex::decode(parent_vs_hex).map_err(|e| AppError::Internal {
            message: format!("Failed to decode verifying share hex: {e}"),
        })?;

        let mut parent_point_bytes = [0u8; 32];
        parent_point_bytes.copy_from_slice(&parent_vs_bytes);
        let parent_point = curve25519_dalek::edwards::CompressedEdwardsY(parent_point_bytes)
            .decompress()
            .ok_or_else(|| AppError::CryptoError {
                message: "Failed to decompress verifying share point".to_string(),
            })?;

        let child_point = parent_point + shift_point;
        let child_point_hex = hex::encode(child_point.compress().to_bytes());

        let child_vs: VerifyingShare =
            serde_json::from_value(serde_json::Value::String(child_point_hex)).map_err(|e| {
                AppError::Internal {
                    message: format!("Failed to deserialize child verifying share: {e}"),
                }
            })?;

        child_verifying_shares.insert(*id, child_vs);
    }

    // Construct child KeyPackage and PublicKeyPackage
    let my_identifier = *key_package.identifier();
    let my_child_verifying_share = child_verifying_shares
        .get(&my_identifier)
        .ok_or_else(|| AppError::Internal {
            message: "Own identifier not found in verifying shares".to_string(),
        })?
        .clone();

    let child_key_package = KeyPackage::new(
        my_identifier,
        child_share,
        my_child_verifying_share,
        child_vk.clone(),
        *key_package.min_signers(),
    );

    let child_public_key_package = PublicKeyPackage::new(child_verifying_shares, child_vk);

    Ok((child_key_package, child_public_key_package))
}
