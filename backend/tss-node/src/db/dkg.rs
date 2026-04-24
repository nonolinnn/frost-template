use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::dkg::{DkgRoundDataRow, KeyShareRow};

/// Fetch the key share for a given DKG session.
pub async fn get_key_share_by_session(
    pool: &PgPool,
    session_id: Uuid,
) -> AppResult<Option<KeyShareRow>> {
    let row = sqlx::query_as::<_, KeyShareRow>(
        r#"
        SELECT id, session_id, key_package, public_key_package, group_public_key, created_at
        FROM key_shares
        WHERE session_id = $1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Fetch the most recent completed key share (any session).
pub async fn get_latest_key_share(pool: &PgPool) -> AppResult<Option<KeyShareRow>> {
    let row = sqlx::query_as::<_, KeyShareRow>(
        r#"
        SELECT id, session_id, key_package, public_key_package, group_public_key, created_at
        FROM key_shares
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Insert a completed key share after DKG Round 3.
pub async fn insert_key_share(
    pool: &PgPool,
    session_id: Uuid,
    key_package: &serde_json::Value,
    public_key_package: &serde_json::Value,
    group_public_key: &str,
) -> AppResult<KeyShareRow> {
    let row = sqlx::query_as::<_, KeyShareRow>(
        r#"
        INSERT INTO key_shares (id, session_id, key_package, public_key_package, group_public_key)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, session_id, key_package, public_key_package, group_public_key, created_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(key_package)
    .bind(public_key_package)
    .bind(group_public_key)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Fetch DKG round data (secret package) for a given session and round.
pub async fn get_round_data(
    pool: &PgPool,
    session_id: Uuid,
    round: i16,
) -> AppResult<Option<DkgRoundDataRow>> {
    let row = sqlx::query_as::<_, DkgRoundDataRow>(
        r#"
        SELECT id, session_id, round, secret_package, created_at
        FROM dkg_round_data
        WHERE session_id = $1 AND round = $2
        "#,
    )
    .bind(session_id)
    .bind(round)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Store intermediate DKG round secret data.
pub async fn insert_round_data(
    pool: &PgPool,
    session_id: Uuid,
    round: i16,
    secret_package: &serde_json::Value,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO dkg_round_data (id, session_id, round, secret_package)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(session_id)
    .bind(round)
    .bind(secret_package)
    .execute(pool)
    .await?;
    Ok(())
}
