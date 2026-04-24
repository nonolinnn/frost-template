use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::signing::SigningNoncesRow;

/// Fetch stored nonces for a signing request.
pub async fn get_nonces(
    pool: &PgPool,
    signing_request_id: Uuid,
) -> AppResult<Option<SigningNoncesRow>> {
    let row = sqlx::query_as::<_, SigningNoncesRow>(
        r#"
        SELECT id, signing_request_id, nonces, created_at
        FROM signing_nonces
        WHERE signing_request_id = $1
        "#,
    )
    .bind(signing_request_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Store signing nonces generated during Round 1.
pub async fn insert_nonces(
    pool: &PgPool,
    signing_request_id: Uuid,
    nonces: &serde_json::Value,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO signing_nonces (id, signing_request_id, nonces)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(signing_request_id)
    .bind(nonces)
    .execute(pool)
    .await?;
    Ok(())
}

/// Delete nonces after they have been consumed in Round 2.
///
/// Nonce reuse would compromise the private key, so consumed nonces
/// must be removed.
pub async fn delete_nonces(
    pool: &PgPool,
    signing_request_id: Uuid,
) -> AppResult<()> {
    sqlx::query(
        r#"
        DELETE FROM signing_nonces
        WHERE signing_request_id = $1
        "#,
    )
    .bind(signing_request_id)
    .execute(pool)
    .await?;
    Ok(())
}
