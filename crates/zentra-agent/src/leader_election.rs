//! Per-target leader election via PostgreSQL session-level advisory locks.
//!
//! Each agent instance that wants to scrape a target calls `try_acquire()`.
//! Exactly one agent wins; all others skip that target.
//!
//! The lock is tied to a dedicated `PgConnection` (not pooled).
//! Dropping the `TargetLock` closes the connection → PostgreSQL automatically
//! releases all session-level advisory locks, so a crashed agent is safe.

use sqlx::{Connection, PgConnection};
use tracing::{debug, warn};
use uuid::Uuid;

/// Held advisory lock for a single target.
///
/// Keep this value alive for the duration of the collection loop.
/// Drop it (or let the async task end) to release the lock.
pub struct TargetLock {
    /// Dedicated connection holding the PG advisory lock session.
    /// Named with `_` prefix to suppress unused-field warnings; the value is
    /// significant because dropping it closes the connection.
    _conn: PgConnection,
    pub target_id: Uuid,
}

/// Try to become the sole scraper for `target_id`.
///
/// Returns `Some(TargetLock)` if this agent wins the election.
/// Returns `None` if another agent already holds the lock, or if the
/// DB connection fails (safe to skip — another agent likely covers it).
pub async fn try_acquire(db_url: &str, target_id: Uuid) -> Option<TargetLock> {
    let key = uuid_to_advisory_key(target_id);

    let mut conn = match PgConnection::connect(db_url).await {
        Ok(c) => c,
        Err(e) => {
            warn!(
                target_id = %target_id,
                error = %e,
                "Leader election: failed to open advisory-lock connection"
            );
            return None;
        }
    };

    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(key)
        .fetch_one(&mut conn)
        .await
        .unwrap_or(false);

    if acquired {
        debug!(
            target_id = %target_id,
            lock_key = key,
            "Leader election: lock acquired — this agent will scrape this target"
        );
        Some(TargetLock { _conn: conn, target_id })
    } else {
        debug!(
            target_id = %target_id,
            lock_key = key,
            "Leader election: lock held by another agent — skipping"
        );
        let _ = conn.close().await;
        None
    }
}

/// Derive a stable i64 advisory lock key from the first 8 bytes of a UUID.
fn uuid_to_advisory_key(id: Uuid) -> i64 {
    let b = id.as_bytes();
    i64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}
