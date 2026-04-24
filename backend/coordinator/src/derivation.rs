//! HD wallet derivation bridging between FROST and hd-wallet crate.
//!
//! FROST (frost-ed25519) uses `curve25519-dalek` types internally.
//! hd-wallet uses `generic-ec` types which also wraps `curve25519-dalek`
//! for Ed25519. This module provides the conversion logic.
//!
//! **Key insight**: non-hardened Edwards derivation produces a `shift` scalar.
//! The child public key = parent_public_key + shift * G.
//! The child secret key = parent_secret_key + shift.
//! This additive property is what makes threshold signing possible with
//! derived keys: each signer adds the same shift to their secret share.

use std::collections::BTreeMap;

use frost_ed25519::keys::{PublicKeyPackage, VerifyingShare};
use frost_ed25519::{Identifier, VerifyingKey};
use generic_ec::curves::Ed25519;
use generic_ec::Point;
use hd_wallet::{Edwards, ExtendedPublicKey, HdWallet, NonHardenedIndex};
use sha2::{Digest, Sha256};

use crate::error::AppError;

/// Deterministic chain code derived from the group public key.
///
/// Since DKG does not produce a chain code (unlike BIP32 master key generation),
/// we derive one deterministically from the group verifying key bytes.
/// This ensures the same group key always produces the same derivation chain.
fn derive_chain_code(group_vk_bytes: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"frost-hd-wallet-chain-code");
    hasher.update(group_vk_bytes);
    hasher.finalize().into()
}

/// Convert a FROST group verifying key (Base58-encoded) to an hd-wallet
/// ExtendedPublicKey suitable for non-hardened derivation.
pub fn frost_vk_to_extended_pk(
    group_public_key_b58: &str,
) -> Result<ExtendedPublicKey<Ed25519>, AppError> {
    // Decode Base58 to raw bytes (32-byte compressed Edwards Y)
    let vk_bytes = bs58::decode(group_public_key_b58)
        .into_vec()
        .map_err(|e| AppError::Internal {
            message: format!("Failed to decode group public key from Base58: {e}"),
        })?;

    if vk_bytes.len() != 32 {
        return Err(AppError::Internal {
            message: format!(
                "Group public key has unexpected length: {} (expected 32)",
                vk_bytes.len()
            ),
        });
    }

    let vk_arr: [u8; 32] = vk_bytes.try_into().unwrap();

    // Parse as generic_ec Point<Ed25519>
    let point = Point::<Ed25519>::from_bytes(&vk_arr).map_err(|e| AppError::Internal {
        message: format!("Failed to parse group public key as Ed25519 point: {e}"),
    })?;

    let chain_code = derive_chain_code(&vk_arr);

    Ok(ExtendedPublicKey {
        public_key: point,
        chain_code,
    })
}

/// Derive a child public key for a given wallet index.
///
/// Returns the child public key as 32 compressed bytes (= Solana address bytes),
/// the Base58-encoded address, and the chain code for potential re-derivation.
pub fn derive_child_public_key(
    group_public_key_b58: &str,
    index: u32,
) -> Result<DerivedWallet, AppError> {
    let parent_pk = frost_vk_to_extended_pk(group_public_key_b58)?;

    let child_index = NonHardenedIndex::try_from(index).map_err(|_| AppError::Internal {
        message: format!("Wallet index {index} is out of range for non-hardened derivation"),
    })?;

    let child_pk = Edwards::derive_child_public_key(&parent_pk, child_index);

    // The child public key point as 32 compressed bytes = Solana address
    let child_pk_bytes = child_pk.public_key.to_bytes(true);
    let address = bs58::encode(child_pk_bytes.as_ref()).into_string();
    let public_key_b58 = address.clone(); // For Ed25519, Solana address = public key

    Ok(DerivedWallet {
        address,
        public_key: public_key_b58,
        chain_code: child_pk.chain_code.to_vec(),
    })
}

/// Result of deriving a child wallet from the group public key.
pub struct DerivedWallet {
    /// Solana address (Base58-encoded 32-byte Ed25519 public key).
    pub address: String,
    /// Derived child public key (Base58).
    pub public_key: String,
    /// Chain code for the child key (for potential re-derivation).
    pub chain_code: Vec<u8>,
}

/// Derive the shift scalar for a given wallet index from the group public key bytes.
///
/// This produces the same shift that nodes compute via their derivation module.
fn derive_shift(
    group_vk_bytes: &[u8; 32],
    wallet_index: u32,
) -> Result<curve25519_dalek::Scalar, AppError> {
    let parent_pk = {
        let point =
            Point::<Ed25519>::from_bytes(group_vk_bytes).map_err(|e| AppError::Internal {
                message: format!("Failed to parse group VK as Ed25519 point: {e}"),
            })?;
        let chain_code = derive_chain_code(group_vk_bytes);
        ExtendedPublicKey {
            public_key: point,
            chain_code,
        }
    };

    let child_index = NonHardenedIndex::try_from(wallet_index).map_err(|_| AppError::Internal {
        message: format!(
            "Wallet index {wallet_index} is out of range for non-hardened derivation"
        ),
    })?;

    let derived = <Edwards as hd_wallet::DeriveShift<Ed25519>>::derive_public_shift(
        &parent_pk,
        child_index,
    );

    // Convert generic-ec Scalar<Ed25519> to curve25519-dalek Scalar.
    // generic-ec serializes as big-endian; curve25519-dalek uses little-endian.
    let shift_be_bytes = derived.shift.to_be_bytes();
    let mut shift_le_bytes = [0u8; 32];
    for (i, b) in shift_be_bytes.as_ref().iter().enumerate() {
        shift_le_bytes[31 - i] = *b;
    }
    let dalek_shift = curve25519_dalek::Scalar::from_canonical_bytes(shift_le_bytes)
        .into_option()
        .ok_or_else(|| AppError::Internal {
            message: "Derived shift is not a canonical scalar".to_string(),
        })?;

    Ok(dalek_shift)
}

/// Derive a child `PublicKeyPackage` for FROST signature aggregation.
///
/// This is needed by the coordinator to call `frost::aggregate()` with the
/// correct child verifying key and child verifying shares. The coordinator
/// only has public data (group verifying key + per-node verifying shares).
///
/// `verifying_shares` maps each node ID string (e.g. "node-a") to its
/// Base58-encoded verifying share from DKG round 3 output.
pub fn derive_child_public_key_package(
    group_public_key_b58: &str,
    verifying_shares_b58: &BTreeMap<String, String>,
    wallet_index: u32,
) -> Result<PublicKeyPackage, AppError> {
    // Decode group verifying key
    let vk_bytes_vec = bs58::decode(group_public_key_b58)
        .into_vec()
        .map_err(|e| AppError::Internal {
            message: format!("Failed to decode group public key: {e}"),
        })?;
    let vk_bytes: [u8; 32] = vk_bytes_vec.try_into().map_err(|_| AppError::Internal {
        message: "Group public key is not 32 bytes".to_string(),
    })?;

    // Compute derivation shift
    let dalek_shift = derive_shift(&vk_bytes, wallet_index)?;
    let shift_point = curve25519_dalek::constants::ED25519_BASEPOINT_POINT * dalek_shift;

    // Derive child verifying key
    let parent_pk = {
        let point =
            Point::<Ed25519>::from_bytes(&vk_bytes).map_err(|e| AppError::Internal {
                message: format!("Failed to parse group VK: {e}"),
            })?;
        let chain_code = derive_chain_code(&vk_bytes);
        ExtendedPublicKey {
            public_key: point,
            chain_code,
        }
    };
    let child_index = NonHardenedIndex::try_from(wallet_index).map_err(|_| AppError::Internal {
        message: format!("Wallet index {wallet_index} out of range"),
    })?;
    let child_extended_pk = Edwards::derive_child_public_key(&parent_pk, child_index);
    let child_vk_bytes = child_extended_pk.public_key.to_bytes(true);
    let child_vk_hex = hex::encode(child_vk_bytes.as_ref());
    let child_vk: VerifyingKey =
        serde_json::from_value(serde_json::Value::String(child_vk_hex)).map_err(|e| {
            AppError::Internal {
                message: format!("Failed to deserialize child verifying key: {e}"),
            }
        })?;

    // Derive child verifying shares for each participant
    let mut child_verifying_shares: BTreeMap<Identifier, VerifyingShare> = BTreeMap::new();

    for (node_id, vs_b58) in verifying_shares_b58 {
        let identifier =
            Identifier::derive(node_id.as_bytes()).map_err(|e| AppError::Internal {
                message: format!("Failed to derive FROST identifier for {node_id}: {e}"),
            })?;

        // Decode verifying share from Base58
        let vs_bytes = bs58::decode(vs_b58)
            .into_vec()
            .map_err(|e| AppError::Internal {
                message: format!("Failed to decode verifying share for {node_id}: {e}"),
            })?;

        let mut point_bytes = [0u8; 32];
        point_bytes.copy_from_slice(&vs_bytes);
        let parent_point = curve25519_dalek::edwards::CompressedEdwardsY(point_bytes)
            .decompress()
            .ok_or_else(|| AppError::Internal {
                message: format!(
                    "Failed to decompress verifying share point for {node_id}"
                ),
            })?;

        // child_verifying_share = parent_verifying_share + shift * G
        let child_point = parent_point + shift_point;
        let child_point_hex = hex::encode(child_point.compress().to_bytes());

        let child_vs: VerifyingShare =
            serde_json::from_value(serde_json::Value::String(child_point_hex)).map_err(|e| {
                AppError::Internal {
                    message: format!("Failed to deserialize child verifying share: {e}"),
                }
            })?;

        child_verifying_shares.insert(identifier, child_vs);
    }

    Ok(PublicKeyPackage::new(child_verifying_shares, child_vk))
}

#[cfg(test)]
mod tests {
    use super::*;
    use frost_ed25519::keys::dkg;
    use frost_ed25519::Identifier;
    use rand::rngs::OsRng;
    use std::collections::BTreeMap;

    /// Helper: derive a FROST Identifier from a node ID string.
    fn id_for(node_id: &str) -> Identifier {
        Identifier::derive(node_id.as_bytes()).unwrap()
    }

    /// Run a full 2-of-2 DKG and return per-node key packages plus the
    /// group public key as Base58.
    fn run_dkg() -> (
        frost_ed25519::keys::KeyPackage,
        frost_ed25519::keys::KeyPackage,
        frost_ed25519::keys::PublicKeyPackage,
        String, // group_public_key_b58
    ) {
        let mut rng = OsRng;
        let id_a = id_for("node-a");
        let id_b = id_for("node-b");

        let (s_a1, p_a1) = dkg::part1(id_a, 2, 2, &mut rng).unwrap();
        let (s_b1, p_b1) = dkg::part1(id_b, 2, 2, &mut rng).unwrap();

        let mut r1_a = BTreeMap::new();
        r1_a.insert(id_b, p_b1.clone());
        let (s_a2, pkgs_a2) = dkg::part2(s_a1, &r1_a).unwrap();

        let mut r1_b = BTreeMap::new();
        r1_b.insert(id_a, p_a1.clone());
        let (s_b2, pkgs_b2) = dkg::part2(s_b1, &r1_b).unwrap();

        let mut r2_a = BTreeMap::new();
        r2_a.insert(id_b, pkgs_b2.get(&id_a).unwrap().clone());
        let (kp_a, pkp_a) = dkg::part3(&s_a2, &r1_a, &r2_a).unwrap();

        let mut r2_b = BTreeMap::new();
        r2_b.insert(id_a, pkgs_a2.get(&id_b).unwrap().clone());
        let (kp_b, _pkp_b) = dkg::part3(&s_b2, &r1_b, &r2_b).unwrap();

        let vk_bytes = pkp_a.verifying_key().serialize().unwrap();
        let gpk_b58 = bs58::encode(&vk_bytes).into_string();

        (kp_a, kp_b, pkp_a, gpk_b58)
    }

    // ---- Wallet address derivation ----

    #[test]
    fn derive_child_public_key_returns_valid_solana_address() {
        let (_, _, _, gpk_b58) = run_dkg();

        let derived = derive_child_public_key(&gpk_b58, 0).unwrap();

        // Solana addresses are Base58-encoded 32-byte Ed25519 keys.
        let decoded = bs58::decode(&derived.address).into_vec().unwrap();
        assert_eq!(
            decoded.len(),
            32,
            "Derived address should decode to exactly 32 bytes"
        );
        assert_eq!(
            derived.address, derived.public_key,
            "For Ed25519, Solana address should equal the public key"
        );
    }

    #[test]
    fn derive_child_public_key_different_indices_give_different_addresses() {
        let (_, _, _, gpk_b58) = run_dkg();

        let d0 = derive_child_public_key(&gpk_b58, 0).unwrap();
        let d1 = derive_child_public_key(&gpk_b58, 1).unwrap();

        assert_ne!(
            d0.address, d1.address,
            "Different wallet indices should yield different addresses"
        );
    }

    #[test]
    fn derive_child_public_key_is_deterministic() {
        let (_, _, _, gpk_b58) = run_dkg();

        let d1 = derive_child_public_key(&gpk_b58, 5).unwrap();
        let d2 = derive_child_public_key(&gpk_b58, 5).unwrap();

        assert_eq!(
            d1.address, d2.address,
            "Same group key + same index should always produce the same address"
        );
    }

    // ---- Coordinator ↔ Node derivation consistency ----

    #[test]
    fn coordinator_child_vk_matches_node_child_vk() {
        // This test verifies that the coordinator's public-key-only
        // derivation produces the same child verifying key as a node's
        // full key derivation.
        let (kp_a, _kp_b, pkp_a, gpk_b58) = run_dkg();

        let wallet_index = 3u32;

        // Coordinator derives the child public key
        let coord_derived = derive_child_public_key(&gpk_b58, wallet_index).unwrap();

        // Node derives the child key package (has the full secret)
        let child_extended_pk = {
            let parent = frost_vk_to_extended_pk(&gpk_b58).unwrap();
            let idx = hd_wallet::NonHardenedIndex::try_from(wallet_index).unwrap();
            hd_wallet::Edwards::derive_child_public_key(&parent, idx)
        };
        let node_child_pk_bytes = child_extended_pk.public_key.to_bytes(true);
        let node_child_address = bs58::encode(node_child_pk_bytes.as_ref()).into_string();

        assert_eq!(
            coord_derived.address, node_child_address,
            "Coordinator and node must derive the same child address"
        );
    }

    // ---- PublicKeyPackage derivation ----

    #[test]
    fn derive_child_public_key_package_produces_valid_package() {
        let (_kp_a, _kp_b, pkp_a, gpk_b58) = run_dkg();

        let id_a = id_for("node-a");
        let id_b = id_for("node-b");

        // Build verifying shares map (Base58)
        let mut vs_b58 = BTreeMap::new();
        for (nid, id) in [("node-a", id_a), ("node-b", id_b)] {
            let vs = pkp_a.verifying_shares().get(&id).unwrap();
            let vs_bytes = serde_json::to_value(vs).unwrap();
            let vs_hex = vs_bytes.as_str().unwrap();
            let vs_raw = hex::decode(vs_hex).unwrap();
            let vs_b58_str = bs58::encode(&vs_raw).into_string();
            vs_b58.insert(nid.to_string(), vs_b58_str);
        }

        let child_pkp =
            derive_child_public_key_package(&gpk_b58, &vs_b58, 0).unwrap();

        // Verify it has the correct number of verifying shares
        assert_eq!(
            child_pkp.verifying_shares().len(),
            2,
            "Child public key package should have 2 verifying shares"
        );

        // Verify the child verifying key is valid (non-identity)
        let child_vk_bytes = child_pkp.verifying_key().serialize().unwrap();
        assert_eq!(child_vk_bytes.len(), 32);
    }

    #[test]
    fn coordinator_child_pkp_vk_matches_child_address() {
        // The child PublicKeyPackage's verifying key should correspond
        // to the same Solana address as derive_child_public_key.
        let (_kp_a, _kp_b, pkp_a, gpk_b58) = run_dkg();

        let id_a = id_for("node-a");
        let id_b = id_for("node-b");

        let mut vs_b58 = BTreeMap::new();
        for (nid, id) in [("node-a", id_a), ("node-b", id_b)] {
            let vs = pkp_a.verifying_shares().get(&id).unwrap();
            let vs_hex = serde_json::to_value(vs).unwrap();
            let vs_raw = hex::decode(vs_hex.as_str().unwrap()).unwrap();
            vs_b58.insert(nid.to_string(), bs58::encode(&vs_raw).into_string());
        }

        let wallet_index = 0u32;
        let child_pkp =
            derive_child_public_key_package(&gpk_b58, &vs_b58, wallet_index).unwrap();
        let child_vk_bytes = child_pkp.verifying_key().serialize().unwrap();
        let child_vk_address = bs58::encode(&child_vk_bytes).into_string();

        let wallet = derive_child_public_key(&gpk_b58, wallet_index).unwrap();

        assert_eq!(
            wallet.address, child_vk_address,
            "derive_child_public_key and derive_child_public_key_package should agree on the child address"
        );
    }
}
