/// Returns the SQL for creating the full Zentra schema.
/// This is idempotent (uses IF NOT EXISTS).
pub fn create_schema_sql() -> &'static str {
    include_str!("../../../../deploy/sql/001_schema.sql")
}

/// Returns the SQL for creating indexes.
pub fn create_indexes_sql() -> &'static str {
    include_str!("../../../../deploy/sql/002_indexes.sql")
}
