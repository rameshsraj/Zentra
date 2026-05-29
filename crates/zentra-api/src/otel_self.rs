//! OpenTelemetry Self-Instrumentation for Zentra
//!
//! Makes Zentra emit its OWN traces and metrics via OTLP so the platform
//! can monitor itself through the same OTel pipeline it provides to customers.
//!
//! Architecture:
//!   1. `init_tracer()` — creates a TracerProvider with an OTLP exporter that
//!      sends spans to Zentra's own `/v1/traces` endpoint (self-loop) or an
//!      external collector.
//!   2. Returns a `tracing_opentelemetry::OpenTelemetryLayer` that plugs into
//!      the existing `tracing_subscriber` stack in `main.rs`.
//!   3. Every `#[tracing::instrument]` span and `tracing::info_span!()` call
//!      automatically becomes an OTel span with trace_id, span_id, attributes.
//!
//! Configuration (env vars):
//!   OTEL_EXPORTER_OTLP_ENDPOINT — default http://127.0.0.1:5006 (self)
//!   OTEL_SERVICE_NAME           — default "zentra-api"
//!   ZENTRA_OTEL_SELF_ENABLED    — default "true"
//!   ZENTRA_OTEL_SAMPLE_RATIO    — default "0.1" (10% sampling)

use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_sdk::{
    trace::{self as sdktrace, Config, Sampler},
    Resource,
};
use opentelemetry_semantic_conventions as semconv;
use tracing::info;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::registry::LookupSpan;

/// Initialise the OTel TracerProvider and return a tracing layer.
///
/// The returned layer should be composed into the subscriber in `main.rs`:
/// ```ignore
/// tracing_subscriber::registry()
///     .with(fmt_layer)
///     .with(otel_layer)   // ← this
///     .init();
/// ```
pub fn init_tracer<S>() -> Option<OpenTelemetryLayer<S, sdktrace::Tracer>>
where
    S: tracing::Subscriber + for<'span> LookupSpan<'span>,
{
    let enabled = std::env::var("ZENTRA_OTEL_SELF_ENABLED")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);

    if !enabled {
        info!("OTel self-instrumentation disabled (ZENTRA_OTEL_SELF_ENABLED=false)");
        return None;
    }

    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:5006".into());

    let service_name = std::env::var("OTEL_SERVICE_NAME")
        .unwrap_or_else(|_| "zentra-api".into());

    let sample_ratio: f64 = std::env::var("ZENTRA_OTEL_SAMPLE_RATIO")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.1);

    // Set the endpoint env var so the OTLP exporter picks it up
    std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint);

    // Build OTLP exporter (HTTP/protobuf to self or external collector)
    let exporter = match opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .build()
    {
        Ok(exp) => exp,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to build OTLP exporter, self-instrumentation disabled");
            return None;
        }
    };

    // Build resource with OTel semantic conventions
    let resource = Resource::new(vec![
        KeyValue::new(semconv::resource::SERVICE_NAME, service_name.clone()),
        KeyValue::new(semconv::resource::SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
        KeyValue::new("service.namespace", "zentra"),
        KeyValue::new("deployment.environment",
            std::env::var("ZENTRA_ENV").unwrap_or_else(|_| "production".into())
        ),
    ]);

    // Build TracerProvider with batch span processor + sampling
    let provider = sdktrace::TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_config(
            Config::default()
                .with_sampler(Sampler::TraceIdRatioBased(sample_ratio))
                .with_resource(resource)
        )
        .build();

    let tracer = provider.tracer("zentra-api");

    // Set as global provider so other crates can access it
    let _ = opentelemetry::global::set_tracer_provider(provider);

    info!(
        endpoint = %endpoint,
        service = %service_name,
        sample_ratio = sample_ratio,
        "OTel self-instrumentation initialized"
    );

    Some(tracing_opentelemetry::layer().with_tracer(tracer))
}

/// Graceful shutdown — flush any pending spans before exit.
pub fn shutdown_tracer() {
    opentelemetry::global::shutdown_tracer_provider();
    info!("OTel tracer provider shut down");
}
