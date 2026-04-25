//! API error-path tests for the Coordinator routes.
//!
//! # Testing Strategy
//!
//! These tests cover input validation and DB-level precondition checks
//! that can be verified without any external services (TSS nodes or Solana RPC).
//!
//! Routes that delegate to TSS Nodes (DKG/signing rounds) or Solana RPC
//! (aggregate/broadcast) are covered by the integration test suite
//! (`tests/integration-test.sh`), which exercises the full happy path
//! against live services. The intentional gap — error paths *within*
//! node calls or RPC calls — would require HTTP mocking (e.g. wiremock)
//! and is out of scope for this demo.
//!
//! # Running
//!
//! Requires a running PostgreSQL instance. With Docker Compose up:
//!
//! ```bash
//! DATABASE_URL=postgresql://frost:frost@localhost:5432/postgres \
//!   cargo test -p coordinator
//! ```
//!
//! `sqlx::test` creates an isolated temporary database per test and
//! cleans it up automatically.

use std::collections::HashMap;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

use crate::config::Config;
use crate::{build_app, AppState};

// ── Helpers ─────────────────────────────────────────────────────────────────

/// A valid 32-byte Solana address (system program). Used as a test recipient.
const VALID_RECIPIENT: &str = "11111111111111111111111111111111";

/// Build an AppState pointing at the test pool.
/// Node URLs are unreachable — error-path tests never reach the node call.
fn test_state(pool: PgPool) -> AppState {
    let mut node_urls = HashMap::new();
    node_urls.insert("node-a".into(), "http://127.0.0.1:19991".into());
    node_urls.insert("node-b".into(), "http://127.0.0.1:19992".into());

    AppState {
        pool,
        config: Config {
            database_url: String::new(),
            port: 0,
            solana_rpc_url: "https://api.devnet.solana.com".into(),
            node_urls,
        },
        http_client: reqwest::Client::new(),
    }
}

/// Build a POST request with a JSON body.
fn post_json(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

/// Build a POST request with no body.
fn post_empty(uri: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

/// Build a GET request.
fn get_req(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

// ── Signing request — input validation ──────────────────────────────────────

/// amount_lamports = 0 must be rejected before any DB or node interaction.
#[sqlx::test]
async fn signing_request_rejects_zero_amount(pool: PgPool) {
    let app = build_app(test_state(pool));
    let resp = app
        .oneshot(post_json(
            "/api/signing-requests",
            json!({
                "wallet_index": 0,
                "recipient": VALID_RECIPIENT,
                "amount_lamports": 0
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// Negative amount must also be rejected.
#[sqlx::test]
async fn signing_request_rejects_negative_amount(pool: PgPool) {
    let app = build_app(test_state(pool));
    let resp = app
        .oneshot(post_json(
            "/api/signing-requests",
            json!({
                "wallet_index": 0,
                "recipient": VALID_RECIPIENT,
                "amount_lamports": -500
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// A recipient that is not valid Base58 must be rejected.
#[sqlx::test]
async fn signing_request_rejects_invalid_recipient(pool: PgPool) {
    let app = build_app(test_state(pool));
    let resp = app
        .oneshot(post_json(
            "/api/signing-requests",
            json!({
                "wallet_index": 0,
                "recipient": "not-valid-base58!@#",
                "amount_lamports": 1000
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// A recipient that decodes to something other than 32 bytes must be rejected.
#[sqlx::test]
async fn signing_request_rejects_short_recipient(pool: PgPool) {
    let app = build_app(test_state(pool));
    // "Gm" decodes to 2 bytes — valid Base58, but not a 32-byte Solana address.
    let resp = app
        .oneshot(post_json(
            "/api/signing-requests",
            json!({
                "wallet_index": 0,
                "recipient": "Gm",
                "amount_lamports": 1000
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ── Signing request — DKG precondition ──────────────────────────────────────

/// Creating a signing request before DKG is complete must return 409 Conflict.
#[sqlx::test]
async fn signing_request_without_dkg_returns_conflict(pool: PgPool) {
    let app = build_app(test_state(pool));
    let resp = app
        .oneshot(post_json(
            "/api/signing-requests",
            json!({
                "wallet_index": 0,
                "recipient": VALID_RECIPIENT,
                "amount_lamports": 1000
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

// ── Signing request — not found ──────────────────────────────────────────────

/// GET /api/signing-requests/{id} with a valid but unknown UUID returns 404.
#[sqlx::test]
async fn get_signing_request_not_found(pool: PgPool) {
    let app = build_app(test_state(pool));
    let unknown_id = uuid::Uuid::new_v4();
    let resp = app
        .oneshot(get_req(&format!("/api/signing-requests/{unknown_id}")))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Wallet creation — DKG precondition ──────────────────────────────────────

/// Deriving a wallet before DKG is complete must return 409 Conflict.
#[sqlx::test]
async fn create_wallet_before_dkg_returns_conflict(pool: PgPool) {
    let app = build_app(test_state(pool));
    let resp = app
        .oneshot(post_empty("/api/wallets"))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

// ── DKG start — idempotency ──────────────────────────────────────────────────

/// Starting DKG a second time while the session is still initializing
/// must return 409 Conflict (DKG_IN_PROGRESS).
#[sqlx::test]
async fn start_dkg_twice_returns_conflict(pool: PgPool) {
    // First call creates the session (status: initialized).
    build_app(test_state(pool.clone()))
        .oneshot(post_empty("/api/dkg/start"))
        .await
        .unwrap();

    // Second call sees the existing session and must reject.
    let resp = build_app(test_state(pool))
        .oneshot(post_empty("/api/dkg/start"))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CONFLICT);
}
