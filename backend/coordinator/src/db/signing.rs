use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::signing::{SigningRequestRow, SigningRoundStateRow};

/// Create a new signing request and pre-populate round state rows for both nodes.
pub async fn create_signing_request(
    pool: &PgPool,
    wallet_index: i32,
    recipient: &str,
    amount_lamports: i64,
) -> AppResult<SigningRequestRow> {
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;

    let row = sqlx::query_as::<_, SigningRequestRow>(
        r#"
        INSERT INTO signing_requests (id, wallet_index, recipient, amount_lamports, status)
        VALUES ($1, $2, $3, $4, 'pending')
        RETURNING id, wallet_index, recipient, amount_lamports, status::text,
                  tx_message, tx_signature, error_message, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(wallet_index)
    .bind(recipient)
    .bind(amount_lamports)
    .fetch_one(&mut *tx)
    .await?;

    // Pre-populate round state rows for both nodes, rounds 1 and 2
    for node_id in &["node-a", "node-b"] {
        for round in 1..=2i16 {
            sqlx::query(
                r#"
                INSERT INTO signing_round_state (id, signing_request_id, node_id, round, status)
                VALUES ($1, $2, $3, $4, 'pending')
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(id)
            .bind(node_id)
            .bind(round)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;

    Ok(row)
}

/// List all signing requests, ordered by creation time (newest first).
pub async fn list_signing_requests(pool: &PgPool) -> AppResult<Vec<SigningRequestRow>> {
    let rows = sqlx::query_as::<_, SigningRequestRow>(
        r#"
        SELECT id, wallet_index, recipient, amount_lamports, status::text,
               tx_message, tx_signature, error_message, created_at, updated_at
        FROM signing_requests
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Fetch a signing request by ID.
pub async fn get_signing_request(pool: &PgPool, id: Uuid) -> AppResult<Option<SigningRequestRow>> {
    let row = sqlx::query_as::<_, SigningRequestRow>(
        r#"
        SELECT id, wallet_index, recipient, amount_lamports, status::text,
               tx_message, tx_signature, error_message, created_at, updated_at
        FROM signing_requests
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Fetch all round state rows for a given signing request.
pub async fn get_signing_round_states(
    pool: &PgPool,
    signing_request_id: Uuid,
) -> AppResult<Vec<SigningRoundStateRow>> {
    let rows = sqlx::query_as::<_, SigningRoundStateRow>(
        r#"
        SELECT id, signing_request_id, node_id, round, status::text,
               output_data, created_at, updated_at
        FROM signing_round_state
        WHERE signing_request_id = $1
        ORDER BY node_id, round
        "#,
    )
    .bind(signing_request_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Fetch a specific signing round state.
pub async fn get_signing_round_state(
    pool: &PgPool,
    signing_request_id: Uuid,
    node_id: &str,
    round: i16,
) -> AppResult<Option<SigningRoundStateRow>> {
    let row = sqlx::query_as::<_, SigningRoundStateRow>(
        r#"
        SELECT id, signing_request_id, node_id, round, status::text,
               output_data, created_at, updated_at
        FROM signing_round_state
        WHERE signing_request_id = $1 AND node_id = $2 AND round = $3
        "#,
    )
    .bind(signing_request_id)
    .bind(node_id)
    .bind(round)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Update a signing round state to complete with output data.
pub async fn complete_signing_round(
    pool: &PgPool,
    signing_request_id: Uuid,
    node_id: &str,
    round: i16,
    output_data: Option<serde_json::Value>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE signing_round_state
        SET status = 'complete', output_data = $4
        WHERE signing_request_id = $1 AND node_id = $2 AND round = $3
        "#,
    )
    .bind(signing_request_id)
    .bind(node_id)
    .bind(round)
    .bind(output_data)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update signing request status.
pub async fn update_signing_request_status(pool: &PgPool, id: Uuid, status: &str) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE signing_requests
        SET status = $2::signing_request_status
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(())
}

/// Store the serialized transaction message on a signing request.
///
/// Called before Round 2 so that the same message bytes are used
/// consistently across all nodes and for aggregation.
pub async fn update_signing_request_tx_message(
    pool: &PgPool,
    id: Uuid,
    tx_message: &[u8],
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE signing_requests
        SET tx_message = $2
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(tx_message)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update signing request with transaction signature after broadcast.
pub async fn update_signing_request_tx(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    tx_signature: Option<&str>,
    error_message: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE signing_requests
        SET status = $2::signing_request_status,
            tx_signature = $3,
            error_message = $4
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(status)
    .bind(tx_signature)
    .bind(error_message)
    .execute(pool)
    .await?;
    Ok(())
}
