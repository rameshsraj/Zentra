//! Cached outbound connection pools for monitored database targets.
//!
//! `resolve_pool` previously created a fresh `PgPool` on every request, causing
//! a connection storm under load. This module wraps a `DashMap` keyed by
//! `target_id` so pools are reused across requests.
//!
//! Eviction: when the cache exceeds `MAX_ENTRIES`, one entry is removed before
//! inserting the new one. TTL-based eviction can be layered later via `moka`.

use dashmap::DashMap;
use sqlx::PgPool;
use std::time::{Duration, Instant};
use std::sync::Arc;
use uuid::Uuid;

const MAX_ENTRIES: usize = 64;
const MAX_IDLE_SECS: u64 = 300; // 5 minutes

struct CachedPool {
    pool: PgPool,
    last_used: Instant,
}

pub struct TargetPoolCache {
    inner: DashMap<Uuid, CachedPool>,
}

impl TargetPoolCache {
    pub fn new() -> Self {
        Self { inner: DashMap::with_capacity(MAX_ENTRIES) }
    }

    /// Return a cached pool for `target_id`, or `None` if not present / expired.
    pub fn get(&self, target_id: Uuid) -> Option<PgPool> {
        let mut entry = self.inner.get_mut(&target_id)?;
        if entry.last_used.elapsed() > Duration::from_secs(MAX_IDLE_SECS) {
            drop(entry);
            self.inner.remove(&target_id);
            return None;
        }
        entry.last_used = Instant::now();
        Some(entry.pool.clone())
    }

    /// Insert a pool, evicting one entry if the cache is at capacity.
    pub fn insert(&self, target_id: Uuid, pool: PgPool) {
        if self.inner.len() >= MAX_ENTRIES {
            // Remove the first (arbitrary) entry to cap memory.
            if let Some(oldest_key) = self.inner.iter().next().map(|e| *e.key()) {
                self.inner.remove(&oldest_key);
            }
        }
        self.inner.insert(target_id, CachedPool { pool, last_used: Instant::now() });
    }

    /// Explicitly evict a target's pool (call when a target is updated/deleted).
    pub fn evict(&self, target_id: Uuid) {
        self.inner.remove(&target_id);
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl Default for TargetPoolCache {
    fn default() -> Self { Self::new() }
}

pub type SharedTargetPoolCache = Arc<TargetPoolCache>;

pub fn new_cache() -> SharedTargetPoolCache {
    Arc::new(TargetPoolCache::new())
}
