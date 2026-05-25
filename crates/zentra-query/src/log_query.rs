use zentra_core::LogEntry;
use zentra_db::DbPool;
use zentra_db::repositories::logs::search_logs;
use super::{TimeRange, Pagination};
use uuid::Uuid;

pub struct LogQueryService {
    pool: DbPool,
}

impl LogQueryService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Full-text search across logs
    pub async fn search(
        &self,
        org_id: Uuid,
        query: Option<&str>,
        time_range: &TimeRange,
        severity: Option<&str>,
        source: Option<&str>,
        target_id: Option<Uuid>,
        pagination: &Pagination,
    ) -> Result<Vec<LogEntry>, zentra_core::Error> {
        search_logs(
            &self.pool,
            org_id,
            query,
            time_range.from,
            time_range.to,
            severity,
            source,
            target_id,
            pagination.limit,
            pagination.offset,
        )
        .await
        .map_err(zentra_core::Error::from)
    }
}
