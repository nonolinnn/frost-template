use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::dkg::{DkgRoundStateRow, DkgSessionRow};

/// Fetch the most recent non-failed DKG session, if any.
pub async fn get_active_session(pool: &PgPool) -> AppResult<Option<DkgSessionRow>> {
    let row = sqlx::query_as::<_, DkgSessionRow>(
        r#"
        SELECT id, status::text, group_public_key, created_at, completed_at, updated_at
        FROM dkg_sessions
        WHERE status != 'failed'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Fetch a DKG session by ID.
pub async fn get_session_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<DkgSessionRow>> {
    let row = sqlx::query_as::<_, DkgSessionRow>(
        r#"
        SELECT id, status::text, group_public_key, created_at, completed_at, updated_at
        FROM dkg_sessions
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Create a new DKG session and pre-populate round state rows for both nodes.
pub async fn create_session(pool: &PgPool) -> AppResult<DkgSessionRow> {
    let session_id = Uuid::new_v4();

    // Insert the session
    let row = sqlx::query_as::<_, DkgSessionRow>(
        r#"
        INSERT INTO dkg_sessions (id, status)
        VALUES ($1, 'initialized')
        RETURNING id, status::text, group_public_key, created_at, completed_at, updated_at
        "#,
    )
    .bind(session_id)
    .fetch_one(pool)
    .await?;

    // Pre-populate round state for all node + round combinations
    for node_id in &["node-a", "node-b"] {
        for round in 1..=3i16 {
            sqlx::query(
                r#"
                INSERT INTO dkg_round_state (id, session_id, node_id, round, status)
                VALUES ($1, $2, $3, $4, 'pending')
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(session_id)
            .bind(node_id)
            .bind(round)
            .execute(pool)
            .await?;
        }
    }

    Ok(row)
}

/// Fetch all round state rows for a given session.
pub async fn get_round_states(
    pool: &PgPool,
    session_id: Uuid,
) -> AppResult<Vec<DkgRoundStateRow>> {
    let rows = sqlx::query_as::<_, DkgRoundStateRow>(
        r#"
        SELECT id, session_id, node_id, round, status::text, output_package,
               created_at, updated_at
        FROM dkg_round_state
        WHERE session_id = $1
        ORDER BY node_id, round
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Fetch a specific round state row for a node in a session.
pub async fn get_round_state(
    pool: &PgPool,
    session_id: Uuid,
    node_id: &str,
    round: i16,
) -> AppResult<Option<DkgRoundStateRow>> {
    let row = sqlx::query_as::<_, DkgRoundStateRow>(
        r#"
        SELECT id, session_id, node_id, round, status::text, output_package,
               created_at, updated_at
        FROM dkg_round_state
        WHERE session_id = $1 AND node_id = $2 AND round = $3
        "#,
    )
    .bind(session_id)
    .bind(node_id)
    .bind(round)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Update the round state for a specific node/round to complete, storing the output package.
pub async fn complete_round(
    pool: &PgPool,
    session_id: Uuid,
    node_id: &str,
    round: i16,
    output_package: Option<serde_json::Value>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE dkg_round_state
        SET status = 'complete', output_package = $4
        WHERE session_id = $1 AND node_id = $2 AND round = $3
        "#,
    )
    .bind(session_id)
    .bind(node_id)
    .bind(round)
    .bind(output_package)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update the DKG session status.
pub async fn update_session_status(
    pool: &PgPool,
    session_id: Uuid,
    status: &str,
    group_public_key: Option<&str>,
) -> AppResult<()> {
    if status == "complete" {
        sqlx::query(
            r#"
            UPDATE dkg_sessions
            SET status = 'complete'::dkg_status,
                group_public_key = $2,
                completed_at = now()
            WHERE id = $1
            "#,
        )
        .bind(session_id)
        .bind(group_public_key)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            r#"
            UPDATE dkg_sessions
            SET status = $2::dkg_status
            WHERE id = $1
            "#,
        )
        .bind(session_id)
        .bind(status)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Check if a completed DKG session exists (for wallet derivation preconditions).
pub async fn get_completed_session(pool: &PgPool) -> AppResult<Option<DkgSessionRow>> {
    let row = sqlx::query_as::<_, DkgSessionRow>(
        r#"
        SELECT id, status::text, group_public_key, created_at, completed_at, updated_at
        FROM dkg_sessions
        WHERE status = 'complete'
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
