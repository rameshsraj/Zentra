use axum::{Router, routing::{get, post, put, delete}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // Alerts
        .route("/api/alerts", get(handlers::alerts::list_alerts))
        .route("/api/alerts/:id/acknowledge", put(handlers::alerts::acknowledge_alert))
        .route("/api/alerts/:id/resolve", put(handlers::alerts::resolve_alert))

        // Alert Rules
        .route("/api/alert-rules", get(handlers::alerts::list_rules))
        .route("/api/alert-rules", post(handlers::alerts::create_rule))
        .route("/api/alert-rules/:id", put(handlers::alerts::update_rule))
        .route("/api/alert-rules/:id", delete(handlers::alerts::delete_rule))

        // Alert Silences
        .route("/api/alerts/silences", get(handlers::alerts::list_silences))
        .route("/api/alerts/silences", post(handlers::alerts::create_silence))
        .route("/api/alerts/silences/:id", delete(handlers::alerts::delete_silence))

        // Alert Templates
        .route("/api/alerts/templates", get(handlers::alerts::list_templates))
        .route("/api/alerts/templates/apply", post(handlers::alerts::apply_template))

        // P19: Alert Grouping + Inhibition
        .route("/api/alerts/groups", get(handlers::alerts::list_alert_groups))
        .route("/api/alerts/inhibitions", get(handlers::alerts::list_inhibitions))
        .route("/api/alerts/inhibitions", post(handlers::alerts::create_inhibition))
        .route("/api/alerts/inhibitions/:id", delete(handlers::alerts::delete_inhibition))

        // P9: Alert Library
        .route("/api/alerts/library", get(handlers::alerts::list_alert_library))
        .route("/api/alerts/library/:name/enable", post(handlers::alerts::enable_library_alert))

        // P10/P11: Mute Timings + Alert Backtesting
        .route("/api/notifications/mute-timings", get(handlers::mute_timings::list_mute_timings))
        .route("/api/notifications/mute-timings", post(handlers::mute_timings::create_mute_timing))
        .route("/api/notifications/mute-timings/:id", delete(handlers::mute_timings::delete_mute_timing))
        .route("/api/alerts/backtest", post(handlers::mute_timings::backtest_alert))

        // Notification Channels
        .route("/api/notifications/channels", get(handlers::notifications::list_channels))
        .route("/api/notifications/channels", post(handlers::notifications::create_channel))
        .route("/api/notifications/channels/:id", put(handlers::notifications::update_channel))
        .route("/api/notifications/channels/:id", delete(handlers::notifications::delete_channel))
        .route("/api/notifications/test", post(handlers::notifications::test_channel))
        .route("/api/notifications/history", get(handlers::notifications::dispatch_history))

        // Notification Routing Policies
        .route("/api/notifications/policies", get(handlers::notifications::list_policies))
        .route("/api/notifications/policies", post(handlers::notifications::create_policy))
        .route("/api/notifications/policies/:id", delete(handlers::notifications::delete_policy))
}
