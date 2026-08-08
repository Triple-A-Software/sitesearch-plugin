use std::{env, net::SocketAddr, sync::Arc};

use axum::{
    Router,
    routing::{get, post},
};
use site_search::{
    AppState,
    api::{admin, dashboard, search, settings},
    create_cms_db, create_db, create_env,
    rewriter::rewriter,
};
use tokio::net::TcpListener;
use tower_http::{
    normalize_path::NormalizePathLayer,
    services::ServeDir,
    trace::{self, TraceLayer},
};
use tracing::Level;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_target(false).compact().init();

    let db = create_db().await;
    let cms_db = create_cms_db().await;
    let state = AppState {
        db,
        cms_db,
        env: Arc::new(create_env()),
    };

    let router = Router::new()
        // Admin UI (static assets)
        .nest_service("/ui", ServeDir::new("ui/dist"))
        // Admin API
        .route("/api/stats", get(admin::route_stats))
        .route("/api/reindex", post(admin::route_reindex))
        .route(
            "/api/settings",
            get(settings::route_get_settings).put(settings::route_update_settings),
        )
        // Dashboard cards (server-rendered HTML)
        .route("/dashboard/top-queries", get(dashboard::dashboard_top_queries))
        .route("/dashboard/no-results", get(dashboard::dashboard_no_results))
        // Public search results page (GET for a bare visit, POST when Neleto
        // proxies the selected layout in the body)
        .route("/search", get(search::page).post(search::page))
        // Public autocomplete (visitor-facing pages route, returns JSON)
        .route("/search/suggest", get(search::suggest))
        // Rewriter (live indexing of rendered pages)
        .route("/rewriter", post(rewriter))
        .layer(NormalizePathLayer::trim_trailing_slash())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(state);

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("failed to bind port");
    println!("site-search listening on {}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
