use axum::{Json, extract::State};

use crate::{
    AppState, database,
    model::{IndexStats, ReindexResult},
    reindex,
    utils::AppResult,
};

/// GET /api/stats — index size, search totals, and top / no-result queries.
pub async fn route_stats(State(state): State<AppState>) -> AppResult<Json<IndexStats>> {
    Ok(Json(database::stats(&state.db).await?))
}

/// POST /api/reindex — reindex every public page from the CMS.
pub async fn route_reindex(State(state): State<AppState>) -> AppResult<Json<ReindexResult>> {
    let indexed = reindex::run_reindex(&state.db, &state.cms_db).await?;
    let total = database::count_indexed(&state.db).await?;
    Ok(Json(ReindexResult { indexed, total }))
}
