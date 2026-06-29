use axum::{Router, routing::{get, post, delete}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // Middleware Dashboard (P5)
        .route("/api/middleware/dashboard", get(handlers::middleware_dashboard::dashboard))

        // Nginx (P5)
        .route("/api/middleware/nginx", get(handlers::middleware_nginx::list_instances))
        .route("/api/middleware/nginx", post(handlers::middleware_nginx::create_instance))
        .route("/api/middleware/nginx/:id/status", get(handlers::middleware_nginx::get_status))
        .route("/api/middleware/nginx/:id/performance-history", get(handlers::middleware_nginx::performance_history))
        .route("/api/middleware/nginx/:id/config", get(handlers::middleware_nginx::get_config))
        .route("/api/middleware/nginx/:id", delete(handlers::middleware_nginx::delete_instance))

        // Apache (P5)
        .route("/api/middleware/apache", get(handlers::middleware_apache::list_instances))
        .route("/api/middleware/apache", post(handlers::middleware_apache::create_instance))
        .route("/api/middleware/apache/:id/status", get(handlers::middleware_apache::get_status))
        .route("/api/middleware/apache/:id/performance-history", get(handlers::middleware_apache::performance_history))
        .route("/api/middleware/apache/:id/scoreboard", get(handlers::middleware_apache::get_scoreboard))
        .route("/api/middleware/apache/:id", delete(handlers::middleware_apache::delete_instance))

        // HAProxy (P6)
        .route("/api/middleware/haproxy", get(handlers::middleware_haproxy::list_instances))
        .route("/api/middleware/haproxy", post(handlers::middleware_haproxy::create_instance))
        .route("/api/middleware/haproxy/:id/status", get(handlers::middleware_haproxy::get_status))
        .route("/api/middleware/haproxy/:id/performance-history", get(handlers::middleware_haproxy::performance_history))
        .route("/api/middleware/haproxy/:id", delete(handlers::middleware_haproxy::delete_instance))

}
// Disabled middleware types (not enabled in this release):
// tomcat, phpfpm, rabbitmq, kafka, elasticsearch, redis, memcached, docker, kubernetes
