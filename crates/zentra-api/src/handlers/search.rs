use axum::{extract::{State, Query}, Json};
use crate::state::AppState;
use crate::error::ApiError;
use crate::tenant::OrgId;
use zentra_query::SearchResult;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<i64>,
}

pub async fn global_search(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<SearchResult>>, ApiError> {
    let results = state.global_search
        .search(org_id, &params.q, params.limit.unwrap_or(20))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(results))
}
