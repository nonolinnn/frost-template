use sqlx::PgPool;

use crate::error::AppResult;
use crate::models::wallet::WalletRow;

/// Fetch all derived wallets, ordered by index.
pub async fn list_wallets(pool: &PgPool) -> AppResult<Vec<WalletRow>> {
    let rows = sqlx::query_as::<_, WalletRow>(
        r#"
        SELECT index, address, public_key, chain_code, created_at
        FROM wallets
        ORDER BY index ASC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Fetch a wallet by its derivation index.
pub async fn get_wallet_by_index(pool: &PgPool, index: i32) -> AppResult<Option<WalletRow>> {
    let row = sqlx::query_as::<_, WalletRow>(
        r#"
        SELECT index, address, public_key, chain_code, created_at
        FROM wallets
        WHERE index = $1
        "#,
    )
    .bind(index)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Get the next available derivation index (max index + 1, or 0 if none exist).
pub async fn next_index(pool: &PgPool) -> AppResult<i32> {
    let row: Option<(Option<i32>,)> =
        sqlx::query_as(r#"SELECT MAX(index) FROM wallets"#)
            .fetch_optional(pool)
            .await?;
    match row {
        Some((Some(max),)) => Ok(max + 1),
        _ => Ok(0),
    }
}

/// Insert a new wallet row.
pub async fn insert_wallet(
    pool: &PgPool,
    index: i32,
    address: &str,
    public_key: &str,
    chain_code: Option<&[u8]>,
) -> AppResult<WalletRow> {
    let row = sqlx::query_as::<_, WalletRow>(
        r#"
        INSERT INTO wallets (index, address, public_key, chain_code)
        VALUES ($1, $2, $3, $4)
        RETURNING index, address, public_key, chain_code, created_at
        "#,
    )
    .bind(index)
    .bind(address)
    .bind(public_key)
    .bind(chain_code)
    .fetch_one(pool)
    .await?;
    Ok(row)
}
