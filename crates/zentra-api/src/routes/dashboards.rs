use axum::{Router, routing::{get, post, put, delete}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // Dashboards
        .route("/api/dashboards", get(handlers::dashboards::list_dashboards))
        .route("/api/dashboards", post(handlers::dashboards::create_dashboard))
        .route("/api/dashboards/import", post(handlers::dashboards::import_dashboard))
        .route("/api/dashboards/:id", get(handlers::dashboards::get_dashboard))
        .route("/api/dashboards/:id", delete(handlers::dashboards::delete_dashboard))
        .route("/api/dashboards/:id/export", get(handlers::dashboards::export_dashboard))

        // Dashboard Folders
        .route("/api/dashboards/folders", get(handlers::dashboards::list_folders))
        .route("/api/dashboards/folders", post(handlers::dashboards::create_folder))
        .route("/api/dashboards/folders/:id", delete(handlers::dashboards::delete_folder))

        // Dashboard Versioning
        .route("/api/dashboards/:id/versions", get(handlers::dashboards::list_versions))
        .route("/api/dashboards/:id/versions/:vid/restore", post(handlers::dashboards::restore_version))

        // Library Panels
        .route("/api/library-panels", get(handlers::dashboards::list_library_panels))
        .route("/api/library-panels", post(handlers::dashboards::create_library_panel))
        .route("/api/library-panels/:id", delete(handlers::dashboards::delete_library_panel))

        // P13: Dashboard Playlists
        .route("/api/dashboards/playlists", get(handlers::dashboards::list_playlists))
        .route("/api/dashboards/playlists", post(handlers::dashboards::create_playlist))
        .route("/api/dashboards/playlists/:id", put(handlers::dashboards::update_playlist))
        .route("/api/dashboards/playlists/:id", delete(handlers::dashboards::delete_playlist))
        .route("/api/dashboards/playlists/:id/next", get(handlers::dashboards::playlist_next))

        // P13: Dashboard Bookmarks
        .route("/api/dashboards/:id/bookmark", post(handlers::dashboards::bookmark_dashboard))
        .route("/api/dashboards/:id/bookmark", delete(handlers::dashboards::unbookmark_dashboard))
        .route("/api/dashboards/bookmarked", get(handlers::dashboards::list_bookmarked))

        // SW P01: Entity-Scoped Dashboards & Templates
        .route("/api/dashboards/for-entity", get(handlers::dashboards::get_dashboards_for_entity))
        .route("/api/dashboard-templates", get(handlers::dashboards::list_dashboard_templates))
        .route("/api/dashboard-templates/:entity_type/instantiate", post(handlers::dashboards::instantiate_template))

        // V5 Prompt 21: Dashboard Overview / Health / Timeline
        .route("/api/dashboard/overview", get(handlers::dashboards::dashboard_overview))
        .route("/api/dashboard/health-summary", get(handlers::dashboards::dashboard_health_summary))
        .route("/api/dashboard/timeline", get(handlers::dashboards::dashboard_timeline))

        // Dashboard Snapshots
        .route("/api/snapshots", get(handlers::query_history::list_snapshots))
        .route("/api/snapshots", post(handlers::query_history::create_snapshot))
        .route("/api/snapshots/:key", get(handlers::query_history::get_snapshot))
        .route("/api/snapshots/:key", delete(handlers::query_history::delete_snapshot))
        .route("/api/snapshots/:key/render", get(handlers::query_history::render_snapshot))

        // Query History
        .route("/api/query-history", get(handlers::query_history::list_query_history))
        .route("/api/query-history", post(handlers::query_history::create_query_history))
        .route("/api/query-history/:id/star", post(handlers::query_history::star_query))
        .route("/api/query-history/:id/unstar", delete(handlers::query_history::unstar_query))
        .route("/api/query-history/:id", delete(handlers::query_history::delete_query_history))
        .route("/api/query-history/:id/comment", put(handlers::query_history::update_comment))
        .route("/api/query-history/starred", get(handlers::query_history::list_starred_queries))
        .route("/api/query-history/frequent", get(handlers::query_history::list_frequent_queries))

        // Annotations
        .route("/api/annotations", get(handlers::annotations::list_annotations))
        .route("/api/annotations", post(handlers::annotations::create_annotation))
        .route("/api/annotations/:id", get(handlers::annotations::get_annotation))
        .route("/api/annotations/:id", put(handlers::annotations::update_annotation))
        .route("/api/annotations/:id", delete(handlers::annotations::delete_annotation))
}
