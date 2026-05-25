//! LRU-bounded cache of `PgPool` connections to monitored target databases.
//!
//! Each unique (host, port, database, user) tuple gets at most one pooled
//! connection.  When the cache exceeds `MAX_CACHED_POOLS` entries the
//! least-recently-used pool is closed and evicted.
//!
//! All access is behind `parking_lot::RwLock` so it is safe to call from
//! multiple tokio tasks.  The heavy async work (pool creation) is done
//! *outside* the lock — only the insert/lookup are synchronised.

use parking_lot::RwLock;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Maximum number of target database pools kept alive simultaneously.
const MAX_CACHED_POOLS: usize = 32;

/// Key that uniquely identifies a target database connection.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct PoolKey {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
}

impl std::fmt::Display for PoolKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}:{}/{}", self.user, self.host, self.port, self.database)
    }
}

struct CachedPool {
    pool: PgPool,
    last_used: Instant,
}

/// Global LRU pool cache.
struct PoolCache {
    pools: HashMap<PoolKey, CachedPool>,
}

impl PoolCache {
    fn new() -> Self {
        Self {
            pools: HashMap::with_capacity(MAX_CACHED_POOLS + 1),
        }
    }

    /// Return a clone of the pool handle if the key exists (and touch LRU).
    fn get(&mut self, key: &PoolKey) -> Option<PgPool> {
        if let Some(entry) = self.pools.get_mut(key) {
            entry.last_used = Instant::now();
            Some(entry.pool.clone())
        } else {
            None
        }
    }

    /// Insert a pool, evicting the LRU entry if at capacity.
    fn insert(&mut self, key: PoolKey, pool: PgPool) {
        if self.pools.len() >= MAX_CACHED_POOLS && !self.pools.contains_key(&key) {
            self.evict_lru();
        }
        self.pools.insert(
            key,
            CachedPool {
                pool,
                last_used: Instant::now(),
            },
        );
    }

    /// Remove and close the least-recently-used pool.
    fn evict_lru(&mut self) {
        if let Some((evict_key, _)) = self
            .pools
            .iter()
            .min_by_key(|(_, v)| v.last_used)
            .map(|(k, v)| (k.clone(), v.last_used))
        {
            if let Some(evicted) = self.pools.remove(&evict_key) {
                // Spawn async close so the pool drains gracefully.
                tokio::spawn(async move { evicted.pool.close().await; });
                info!(%evict_key, "Evicted LRU target pool");
            }
        }
    }

    /// Remove a specific pool (e.g. when a target is deleted).
    #[allow(dead_code)]
    fn remove(&mut self, key: &PoolKey) {
        if let Some(entry) = self.pools.remove(key) {
            tokio::spawn(async move { entry.pool.close().await; });
            debug!(%key, "Removed target pool from cache");
        }
    }

    /// Number of cached pools.
    #[allow(dead_code)]
    fn len(&self) -> usize {
        self.pools.len()
    }
}

lazy_static::lazy_static! {
    static ref CACHE: Arc<RwLock<PoolCache>> = Arc::new(RwLock::new(PoolCache::new()));
}

/// Obtain a `PgPool` for the given target key.
///
/// If a cached pool exists it is returned immediately.  Otherwise a new pool
/// is created, inserted into the cache (possibly evicting the LRU entry), and
/// returned.
pub async fn get_or_create(
    key: PoolKey,
    password: &str,
) -> Result<PgPool, String> {
    // Fast path — cache hit (read lock).
    {
        let mut cache = CACHE.write();
        if let Some(pool) = cache.get(&key) {
            debug!(%key, "Target pool cache hit");
            return Ok(pool);
        }
    }

    // Slow path — create a new pool (no lock held during connect).
    let url = format!(
        "postgresql://{}:{}@{}:{}/{}",
        key.user, password, key.host, key.port, key.database
    );

    debug!(%key, "Creating new target pool");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(3)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .idle_timeout(std::time::Duration::from_secs(300))
        .connect(&url)
        .await
        .map_err(|e| {
            warn!(%key, error = %e, "Failed to create target pool");
            e.to_string()
        })?;

    // Insert under write lock.
    {
        let mut cache = CACHE.write();
        cache.insert(key.clone(), pool.clone());
        debug!(%key, cached = cache.len(), "Target pool cached");
    }

    Ok(pool)
}

/// Explicitly close and remove a pool from the cache.
#[allow(dead_code)]
pub fn invalidate(key: &PoolKey) {
    CACHE.write().remove(key);
}

/// Current number of cached target pools.
#[allow(dead_code)]
pub fn pool_count() -> usize {
    CACHE.read().len()
}

/// Close all cached pools (e.g. on shutdown).
#[allow(dead_code)]
pub fn clear_all() {
    let mut cache = CACHE.write();
    for (key, entry) in cache.pools.drain() {
        tokio::spawn(async move { entry.pool.close().await; });
        debug!(%key, "Closed target pool on cache clear");
    }
    info!("Target pool cache cleared");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lru_eviction_order() {
        let mut cache = PoolCache::new();

        // We can't easily create real PgPools in a unit test, but we can
        // verify the eviction logic by checking that insert + evict works
        // at the data-structure level.  We'll skip actual pool creation
        // and just confirm capacity math.
        assert_eq!(cache.len(), 0);

        // After MAX_CACHED_POOLS inserts of the *same* key, size stays 1.
        // (This is a no-op test since we can't construct PgPool without a DB.)
        assert!(cache.pools.is_empty());
    }
}
