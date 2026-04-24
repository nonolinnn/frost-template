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

#[cfg(test)]
mod tests {
    use super::*;
    use frost_ed25519::keys::dkg;
    use frost_ed25519::{self as frost, Identifier, SigningPackage};
    use rand::rngs::OsRng;
    use std::collections::BTreeMap;

    /// Helper: derive a FROST Identifier from a node ID string,
    /// matching the convention used throughout the codebase.
    fn id_for(node_id: &str) -> Identifier {
        Identifier::derive(node_id.as_bytes()).unwrap()
    }

    /// Run a full 2-of-2 DKG between two participants, returning
    /// each participant's (KeyPackage, PublicKeyPackage).
    fn run_dkg() -> (
        (KeyPackage, PublicKeyPackage),
        (KeyPackage, PublicKeyPackage),
    ) {
        let mut rng = OsRng;
        let id_a = id_for("node-a");
        let id_b = id_for("node-b");

        // ---- Round 1 ----
        let (secret_a1, package_a1) = dkg::part1(id_a, 2, 2, &mut rng).unwrap();
        let (secret_b1, package_b1) = dkg::part1(id_b, 2, 2, &mut rng).unwrap();

        // ---- Round 2 ----
        let mut r1_for_a = BTreeMap::new();
        r1_for_a.insert(id_b, package_b1.clone());
        let (secret_a2, packages_a2) = dkg::part2(secret_a1, &r1_for_a).unwrap();

        let mut r1_for_b = BTreeMap::new();
        r1_for_b.insert(id_a, package_a1.clone());
        let (secret_b2, packages_b2) = dkg::part2(secret_b1, &r1_for_b).unwrap();

        // ---- Round 3 ----
        let mut r2_for_a = BTreeMap::new();
        r2_for_a.insert(id_b, packages_b2.get(&id_a).unwrap().clone());
        let (kp_a, pkp_a) = dkg::part3(&secret_a2, &r1_for_a, &r2_for_a).unwrap();

        let mut r2_for_b = BTreeMap::new();
        r2_for_b.insert(id_a, packages_a2.get(&id_b).unwrap().clone());
        let (kp_b, pkp_b) = dkg::part3(&secret_b2, &r1_for_b, &r2_for_b).unwrap();

        ((kp_a, pkp_a), (kp_b, pkp_b))
    }

    // ---- Test 1: DKG round-trip correctness ----

    #[test]
    fn dkg_produces_matching_group_public_keys() {
        let ((_, pkp_a), (_, pkp_b)) = run_dkg();

        // Both nodes must agree on the group verifying key.
        let vk_a = pkp_a.verifying_key();
        let vk_b = pkp_b.verifying_key();

        let vk_a_bytes = vk_a.serialize().unwrap();
        let vk_b_bytes = vk_b.serialize().unwrap();

        assert_eq!(
            vk_a_bytes, vk_b_bytes,
            "Both nodes should derive the same group verifying key from DKG"
        );
    }

    #[test]
    fn dkg_produces_consistent_verifying_shares() {
        let ((_, pkp_a), (_, pkp_b)) = run_dkg();

        let id_a = id_for("node-a");
        let id_b = id_for("node-b");

        // Node A's view of node-B's verifying share should match
        // node B's view of node-B's verifying share, and vice versa.
        let a_view_of_b = pkp_a.verifying_shares().get(&id_b).unwrap();
        let b_view_of_b = pkp_b.verifying_shares().get(&id_b).unwrap();
        assert_eq!(
            serde_json::to_value(a_view_of_b).unwrap(),
            serde_json::to_value(b_view_of_b).unwrap(),
            "Both nodes should agree on node-b's verifying share"
        );

        let a_view_of_a = pkp_a.verifying_shares().get(&id_a).unwrap();
        let b_view_of_a = pkp_b.verifying_shares().get(&id_a).unwrap();
        assert_eq!(
            serde_json::to_value(a_view_of_a).unwrap(),
            serde_json::to_value(b_view_of_a).unwrap(),
            "Both nodes should agree on node-a's verifying share"
        );
    }

    // ---- Test 2: Wallet derivation consistency ----

    #[test]
    fn child_key_derivation_produces_consistent_verifying_keys() {
        let ((kp_a, pkp_a), (kp_b, pkp_b)) = run_dkg();

        let wallet_index = 0u32;

        // Both nodes derive child key packages
        let (child_kp_a, child_pkp_a) =
            derive_child_key_package(&kp_a, &pkp_a, wallet_index).unwrap();
        let (child_kp_b, child_pkp_b) =
            derive_child_key_package(&kp_b, &pkp_b, wallet_index).unwrap();

        // Child verifying keys must match
        let child_vk_a = child_pkp_a.verifying_key();
        let child_vk_b = child_pkp_b.verifying_key();

        let child_vk_a_bytes = child_vk_a.serialize().unwrap();
        let child_vk_b_bytes = child_vk_b.serialize().unwrap();

        assert_eq!(
            child_vk_a_bytes, child_vk_b_bytes,
            "Both nodes should derive the same child verifying key for wallet index {wallet_index}"
        );

        // The key package's verifying key should also match
        assert_eq!(
            child_kp_a.verifying_key().serialize().unwrap(),
            child_vk_a_bytes,
            "Node A's child key package verifying key should match the public key package"
        );
        assert_eq!(
            child_kp_b.verifying_key().serialize().unwrap(),
            child_vk_b_bytes,
            "Node B's child key package verifying key should match the public key package"
        );
    }

    #[test]
    fn child_key_derivation_different_indices_produce_different_keys() {
        let ((kp_a, pkp_a), _) = run_dkg();

        let (_, child_pkp_0) = derive_child_key_package(&kp_a, &pkp_a, 0).unwrap();
        let (_, child_pkp_1) = derive_child_key_package(&kp_a, &pkp_a, 1).unwrap();

        let vk_0 = child_pkp_0.verifying_key().serialize().unwrap();
        let vk_1 = child_pkp_1.verifying_key().serialize().unwrap();

        assert_ne!(
            vk_0, vk_1,
            "Different wallet indices should produce different child verifying keys"
        );
    }

    #[test]
    fn child_key_derivation_is_deterministic() {
        let ((kp_a, pkp_a), _) = run_dkg();

        let (_, child_pkp_first) = derive_child_key_package(&kp_a, &pkp_a, 42).unwrap();
        let (_, child_pkp_second) = derive_child_key_package(&kp_a, &pkp_a, 42).unwrap();

        assert_eq!(
            child_pkp_first.verifying_key().serialize().unwrap(),
            child_pkp_second.verifying_key().serialize().unwrap(),
            "Derivation with the same index should be deterministic"
        );
    }

    // ---- Test 3: Threshold signing correctness ----

    #[test]
    fn threshold_signing_with_root_keys_produces_valid_signature() {
        let ((kp_a, pkp_a), (kp_b, _pkp_b)) = run_dkg();

        let mut rng = OsRng;
        let message = b"test message for FROST threshold signing";

        // Round 1: generate nonces and commitments
        let (nonces_a, commitments_a) =
            frost::round1::commit(kp_a.signing_share(), &mut rng);
        let (nonces_b, commitments_b) =
            frost::round1::commit(kp_b.signing_share(), &mut rng);

        // Build commitments map
        let mut commitments_map = BTreeMap::new();
        commitments_map.insert(*kp_a.identifier(), commitments_a);
        commitments_map.insert(*kp_b.identifier(), commitments_b);

        // Create signing package
        let signing_package = SigningPackage::new(commitments_map, message);

        // Round 2: each node produces a signature share
        let sig_share_a =
            frost::round2::sign(&signing_package, &nonces_a, &kp_a).unwrap();
        let sig_share_b =
            frost::round2::sign(&signing_package, &nonces_b, &kp_b).unwrap();

        // Aggregate
        let mut sig_shares = BTreeMap::new();
        sig_shares.insert(*kp_a.identifier(), sig_share_a);
        sig_shares.insert(*kp_b.identifier(), sig_share_b);

        let group_signature =
            frost::aggregate(&signing_package, &sig_shares, &pkp_a).unwrap();

        // Verify the aggregated signature against the group verifying key
        let vk = pkp_a.verifying_key();
        assert!(
            vk.verify(message, &group_signature).is_ok(),
            "Aggregated FROST signature should verify against the group verifying key"
        );
    }

    #[test]
    fn threshold_signing_with_derived_child_keys_produces_valid_signature() {
        let ((kp_a, pkp_a), (kp_b, pkp_b)) = run_dkg();

        let wallet_index = 7u32;
        let mut rng = OsRng;
        let message = b"signing with a derived child key at index 7";

        // Derive child key packages
        let (child_kp_a, child_pkp_a) =
            derive_child_key_package(&kp_a, &pkp_a, wallet_index).unwrap();
        let (child_kp_b, _child_pkp_b) =
            derive_child_key_package(&kp_b, &pkp_b, wallet_index).unwrap();

        // Round 1: generate nonces and commitments using child signing shares
        let (nonces_a, commitments_a) =
            frost::round1::commit(child_kp_a.signing_share(), &mut rng);
        let (nonces_b, commitments_b) =
            frost::round1::commit(child_kp_b.signing_share(), &mut rng);

        // Build commitments map
        let mut commitments_map = BTreeMap::new();
        commitments_map.insert(*child_kp_a.identifier(), commitments_a);
        commitments_map.insert(*child_kp_b.identifier(), commitments_b);

        // Create signing package
        let signing_package = SigningPackage::new(commitments_map, message);

        // Round 2
        let sig_share_a =
            frost::round2::sign(&signing_package, &nonces_a, &child_kp_a).unwrap();
        let sig_share_b =
            frost::round2::sign(&signing_package, &nonces_b, &child_kp_b).unwrap();

        // Aggregate using the child public key package
        let mut sig_shares = BTreeMap::new();
        sig_shares.insert(*child_kp_a.identifier(), sig_share_a);
        sig_shares.insert(*child_kp_b.identifier(), sig_share_b);

        let group_signature =
            frost::aggregate(&signing_package, &sig_shares, &child_pkp_a).unwrap();

        // Verify the aggregated signature against the child verifying key
        let child_vk = child_pkp_a.verifying_key();
        assert!(
            child_vk.verify(message, &group_signature).is_ok(),
            "Aggregated FROST signature with derived child keys should verify"
        );
    }

    #[test]
    fn threshold_signing_signature_is_64_bytes() {
        let ((kp_a, pkp_a), (kp_b, _)) = run_dkg();

        let mut rng = OsRng;
        let message = b"check signature byte length";

        let (nonces_a, commitments_a) =
            frost::round1::commit(kp_a.signing_share(), &mut rng);
        let (nonces_b, commitments_b) =
            frost::round1::commit(kp_b.signing_share(), &mut rng);

        let mut commitments_map = BTreeMap::new();
        commitments_map.insert(*kp_a.identifier(), commitments_a);
        commitments_map.insert(*kp_b.identifier(), commitments_b);

        let signing_package = SigningPackage::new(commitments_map, message);

        let sig_share_a =
            frost::round2::sign(&signing_package, &nonces_a, &kp_a).unwrap();
        let sig_share_b =
            frost::round2::sign(&signing_package, &nonces_b, &kp_b).unwrap();

        let mut sig_shares = BTreeMap::new();
        sig_shares.insert(*kp_a.identifier(), sig_share_a);
        sig_shares.insert(*kp_b.identifier(), sig_share_b);

        let group_signature =
            frost::aggregate(&signing_package, &sig_shares, &pkp_a).unwrap();

        // Ed25519 signatures are 64 bytes (R || s)
        let sig_bytes = group_signature.serialize().unwrap();
        assert_eq!(
            sig_bytes.len(),
            64,
            "Ed25519 FROST signature should be exactly 64 bytes"
        );
    }

    #[test]
    fn threshold_signing_wrong_message_fails_verification() {
        let ((kp_a, pkp_a), (kp_b, _)) = run_dkg();

        let mut rng = OsRng;
        let message = b"the real message";
        let wrong_message = b"a different message";

        let (nonces_a, commitments_a) =
            frost::round1::commit(kp_a.signing_share(), &mut rng);
        let (nonces_b, commitments_b) =
            frost::round1::commit(kp_b.signing_share(), &mut rng);

        let mut commitments_map = BTreeMap::new();
        commitments_map.insert(*kp_a.identifier(), commitments_a);
        commitments_map.insert(*kp_b.identifier(), commitments_b);

        let signing_package = SigningPackage::new(commitments_map, message);

        let sig_share_a =
            frost::round2::sign(&signing_package, &nonces_a, &kp_a).unwrap();
        let sig_share_b =
            frost::round2::sign(&signing_package, &nonces_b, &kp_b).unwrap();

        let mut sig_shares = BTreeMap::new();
        sig_shares.insert(*kp_a.identifier(), sig_share_a);
        sig_shares.insert(*kp_b.identifier(), sig_share_b);

        let group_signature =
            frost::aggregate(&signing_package, &sig_shares, &pkp_a).unwrap();

        // Verification against the wrong message should fail
        let vk = pkp_a.verifying_key();
        assert!(
            vk.verify(wrong_message, &group_signature).is_err(),
            "Signature should not verify against a different message"
        );
    }
}
