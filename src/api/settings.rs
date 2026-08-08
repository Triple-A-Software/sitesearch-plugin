use axum::{Json, extract::State};

use crate::{AppState, database, model::SearchSettings, utils::AppResult};

/// GET /api/settings
pub async fn route_get_settings(State(state): State<AppState>) -> AppResult<Json<SearchSettings>> {
    Ok(Json(database::get_settings(&state.db).await?))
}

/// PUT /api/settings
pub async fn route_update_settings(
    State(state): State<AppState>,
    Json(body): Json<SearchSettings>,
) -> AppResult<Json<SearchSettings>> {
    let settings = SearchSettings {
        title_weight: body.title_weight.clamp(0.0, 10.0),
        body_weight: body.body_weight.clamp(0.0, 10.0),
        synonyms: sanitize_synonyms(body.synonyms),
    };
    database::update_settings(&state.db, &settings).await?;
    Ok(Json(database::get_settings(&state.db).await?))
}

/// Trim terms, drop blanks, and keep only groups with at least two members (a
/// one-term "group" expands to nothing).
fn sanitize_synonyms(groups: Vec<Vec<String>>) -> Vec<Vec<String>> {
    groups
        .into_iter()
        .map(|group| {
            group
                .into_iter()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|group| group.len() >= 2)
        .collect()
}
