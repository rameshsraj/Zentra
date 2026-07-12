//! Shared OpenTelemetry Self-Instrumentation for Zentra
//!
//! Items 5, 9, 11, 12, 13, 14 from TODO_OTEL_INTEGRATION_GAPS.md
//!
//! Provides `init_telemetry(service_name)` and `shutdown_telemetry()` for all
//! binaries in the workspace. Every binary calls `init_telemetry` from its
//! `main()` and gets a TracerProvider + MeterProvider configured uniformly.
//!
//! Configuration (env vars):
//!   OTEL_EXPORTER_OTLP_ENDPOINT   — default http://127.0.0.1:5006
//!   OTEL_SERVICE_NAME              — override (fallback = `service_name` arg)
//!   OTEL_RESOURCE_ATTRIBUTES       — extra key=value pairs
//!   ZENTRA_OTEL_SELF_ENABLED       — default "true"
//!   ZENTRA_OTEL_SAMPLE_RATIO       — default "0.1" (10%)
//!   ZENTRA_ENV                     — deployment.environment (default "dev")
//!   ZENTRA_OTEL_STDOUT_LOGS        — keep fmt layer for local dev (default "true")

use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_sdk::{
    trace::{self as sdktrace, Config, Sampler},
    Resource,
};
use opentelemetry_semantic_conventions as semconv;
use tracing::{info, warn};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::registry::LookupSpan;
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};

static INITIALIZED: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Item 14: Cardinality Guard
// ============================================================================

/// Bounded-cardinality label wrapper.
/// When the unique-value count for a key exceeds `max_unique`, returns `"_other"`.
pub fn bounded_label<'a>(key: &str, value: &'a str, max_unique: usize) -> Cow<'a, str> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::sync::LazyLock;

    static COUNTERS: LazyLock<Mutex<HashMap<String, std::collections::HashSet<String>>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    let cap = std::env::var(format!("ZENTRA_METRICS_CARDINALITY_CAP_{}", key.to_uppercase().replace('.', "_")))
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(max_unique);

    let mut map = COUNTERS.lock().expect("cardinality lock");
    let set = map.entry(key.to_string()).or_default();

    if set.contains(value) {
        return Cow::Borrowed(value);
    }

    if set.len() >= cap {
        metrics::counter!("zentra.metrics.cardinality.dropped", "key" => key.to_string()).increment(1);
        return Cow::Borrowed("_other");
    }

    set.insert(value.to_string());
    Cow::Borrowed(value)
}

// ============================================================================
// Item 5 + 9: Shared init_telemetry
// ============================================================================

/// Initialise shared OTel TracerProvider and return a tracing layer.
///
/// Call from every binary's `main()`:
/// ```ignore
/// tracing_subscriber::registry()
///     .with(fmt_layer)
///     .with(zentra_core::telemetry::init_telemetry("zentra-agent"))
///     .init();
/// ```
///
/// Item 9: Always attaches uniform resource attributes:
///   service.name (required), service.version, service.namespace=zentra,
///   service.instance.id (UUID v7), deployment.environment
pub fn init_telemetry<S>(service_name: &str) -> Option<OpenTelemetryLayer<S, sdktrace::Tracer>>
where
    S: tracing::Subscriber + for<'span> LookupSpan<'span>,
{
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        tracing::warn!("init_telemetry called more than once — skipping");
        return None;
    }

    let enabled = std::env::var("ZENTRA_OTEL_SELF_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);

    if !enabled {
        info!("OTel self-instrumentation disabled (ZENTRA_OTEL_SELF_ENABLED=false)");
        return None;
    }

    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:5006".into());

    let has_otlp_headers = std::env::var("OTEL_EXPORTER_OTLP_HEADERS")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let exports_to_local_api = endpoint.trim_end_matches('/') == "http://127.0.0.1:5006";
    if exports_to_local_api && !has_otlp_headers {
        warn!(
            endpoint = %endpoint,
            "OTel self-instrumentation disabled: local OTLP ingest requires OTEL_EXPORTER_OTLP_HEADERS with an API key"
        );
        return None;
    }

    let final_service_name = std::env::var("OTEL_SERVICE_NAME")
        .unwrap_or_else(|_| service_name.to_string());

    let sample_ratio: f64 = std::env::var("ZENTRA_OTEL_SAMPLE_RATIO")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.1);

    let environment = std::env::var("ZENTRA_ENV")
        .unwrap_or_else(|_| "dev".into());

    // Set the endpoint env var so the OTLP exporter picks it up
    std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint);

    // Build OTLP exporter (HTTP/protobuf to self or external collector)
    let exporter = match opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .build()
    {
        Ok(exp) => exp,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to build OTLP span exporter, self-instrumentation disabled");
            return None;
        }
    };

    // Item 9: Uniform resource attributes across all services
    let instance_id = uuid::Uuid::now_v7().to_string();
    let mut attrs = vec![
        KeyValue::new(semconv::resource::SERVICE_NAME, final_service_name.clone()),
        KeyValue::new(semconv::resource::SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
        KeyValue::new("service.namespace", "zentra"),
        KeyValue::new("service.instance.id", instance_id),
        KeyValue::new("deployment.environment", environment),
    ];

    // Parse OTEL_RESOURCE_ATTRIBUTES for user extensions
    if let Ok(extra) = std::env::var("OTEL_RESOURCE_ATTRIBUTES") {
        for pair in extra.split(',') {
            if let Some((k, v)) = pair.split_once('=') {
                attrs.push(KeyValue::new(k.trim().to_string(), v.trim().to_string()));
            }
        }
    }

    let resource = Resource::new(attrs);

    // Item 11: ParentBased sampler so child decisions follow parents
    let sampler = Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(sample_ratio)));

    // Build TracerProvider with batch span processor + sampling
    let provider = sdktrace::TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_config(
            Config::default()
                .with_sampler(sampler)
                .with_resource(resource)
        )
        .build();

    let tracer = provider.tracer(final_service_name.clone());

    // Set as global provider so other crates can access it
    let _ = opentelemetry::global::set_tracer_provider(provider);

    info!(
        endpoint = %endpoint,
        service = %final_service_name,
        sample_ratio = sample_ratio,
        "OTel self-instrumentation initialized (shared telemetry)"
    );

    Some(tracing_opentelemetry::layer().with_tracer(tracer))
}

// ============================================================================
// Item 12: OTEL Log Records from tracing
// ============================================================================

/// Initialise an OTLP log exporter and return a tracing layer that bridges
/// `tracing` events (info!, warn!, error!) to OTLP LogRecords.
///
/// Usage:
/// ```ignore
/// tracing_subscriber::registry()
///     .with(zentra_core::telemetry::init_telemetry("svc"))
///     .with(zentra_core::telemetry::init_log_bridge())
///     .init();
/// ```
pub fn init_log_bridge() -> Option<opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge<opentelemetry_sdk::logs::LoggerProvider, opentelemetry_sdk::logs::Logger>> {
    let enabled = std::env::var("ZENTRA_OTEL_SELF_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);

    if !enabled {
        return None;
    }

    let log_exporter = match opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .build()
    {
        Ok(exp) => exp,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to build OTLP log exporter");
            return None;
        }
    };

    let provider = opentelemetry_sdk::logs::LoggerProvider::builder()
        .with_batch_exporter(log_exporter, opentelemetry_sdk::runtime::Tokio)
        .build();

    let bridge = opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&provider);

    info!("OTel log bridge initialized — tracing events will be exported as OTLP LogRecords");

    Some(bridge)
}

// ============================================================================
// Item 6: Shared meter
// ============================================================================

/// Return a named Meter from the global MeterProvider.
/// Usage: `let m = zentra_core::telemetry::meter("zentra-stream");`
pub fn meter(name: &'static str) -> opentelemetry::metrics::Meter {
    opentelemetry::global::meter(name)
}

// ============================================================================
// Item 13: Graceful shutdown
// ============================================================================

/// Flush pending spans/metrics/logs before exit.
/// Call from ctrl_c / SIGTERM handler in every binary.
pub fn shutdown_telemetry() {
    opentelemetry::global::shutdown_tracer_provider();
    info!("OTel telemetry shut down (shared)");
}

/// Install ctrl_c + SIGTERM handler that calls `shutdown_telemetry()`.
/// Returns a future that resolves when the signal is received.
pub async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to register SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        // Windows: ctrl_c covers both Ctrl+C and Ctrl+Break
        let _ = ctrl_c.await;
    }

    info!("Shutdown signal received, flushing telemetry...");
    shutdown_telemetry();
}
