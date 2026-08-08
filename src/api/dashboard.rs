use axum::{extract::State, response::Html};
use minijinja::context;

use crate::{AppState, database, utils::AppResult};

/// GET /dashboard/top-queries — server-rendered dashboard card.
pub async fn dashboard_top_queries(State(state): State<AppState>) -> AppResult<Html<String>> {
    let stats = database::stats(&state.db).await?;
    let tmpl = state.env.get_template("dashboard_top_queries.html")?;
    Ok(Html(tmpl.render(context! {
        queries => stats.top_queries,
        searches => stats.searches,
        indexed => stats.indexed,
    })?))
}

/// GET /dashboard/no-results — server-rendered dashboard card.
pub async fn dashboard_no_results(State(state): State<AppState>) -> AppResult<Html<String>> {
    let stats = database::stats(&state.db).await?;
    let tmpl = state.env.get_template("dashboard_no_results.html")?;
    Ok(Html(tmpl.render(context! {
        queries => stats.no_result_queries,
        total => stats.no_result_searches,
    })?))
}
