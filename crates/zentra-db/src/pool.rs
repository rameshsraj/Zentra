use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Executor};
use tracing::info;

pub type DbPool = PgPool;

/// Pool kind — controls per-connection statement_timeout.
pub enum PoolKind {
    /// Short timeout (5 s). For metadata reads/writes that should be fast.
    Meta,
    /// Long timeout (60 s). For TSDB / analytical queries.
    Tsdb,
}

impl PoolKind {
    fn statement_timeout_ms(&self) -> u32 {
        match self {
            PoolKind::Meta => 5_000,
            PoolKind::Tsdb => 60_000,
        }
    }
    fn max_connections(&self) -> u32 {
        match self {
            PoolKind::Meta => 20,
            PoolKind::Tsdb => 10,
        }
    }
}

/// Create a connection pool with a per-connection `statement_timeout`.
pub async fn create_pool_with_kind(database_url: &str, kind: PoolKind) -> Result<DbPool, sqlx::Error> {
    let timeout_ms = kind.statement_timeout_ms();
    let max_conn = kind.max_connections();
    let pool = PgPoolOptions::new()
        .max_connections(max_conn)
        .min_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .after_connect(move |conn, _meta| {
            Box::pin(async move {
                let _ = conn
                    .execute(format!("SET statement_timeout = {}", timeout_ms).as_str())
                    .await;
                Ok(())
            })
        })
        .connect(database_url)
        .await?;
    info!(timeout_ms, max_conn, "Database pool created");
    Ok(pool)
}

/// Backwards-compatible default — Meta pool (5 s statement timeout, 20 conns).
pub async fn create_pool(database_url: &str) -> Result<DbPool, sqlx::Error> {
    create_pool_with_kind(database_url, PoolKind::Meta).await
}

/// Run health check query
pub async fn health_check(pool: &DbPool) -> bool {
    sqlx::query("SELECT 1")
        .execute(pool)
        .await
        .is_ok()
}
