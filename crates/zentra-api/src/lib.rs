#![recursion_limit = "512"]

pub mod error;
pub mod routes;
pub mod handlers;
pub mod ws;
pub mod state;
pub mod validation;
pub mod provisioning;
pub mod middleware;
pub mod auth_middleware;
pub mod tenant;
pub mod target_pool;
pub mod openapi;
pub mod observability;
pub mod semconv;
pub mod background_jobs;
pub mod license_middleware;

#[cfg(feature = "otlp-proto")]
pub mod otlp_proto;

#[cfg(feature = "grpc")]
pub mod grpc;
