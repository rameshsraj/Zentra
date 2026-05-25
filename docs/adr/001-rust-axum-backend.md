# ADR 001: Rust + Axum for Backend

## Status: Accepted
## Date: 2025-01-01

## Context
Zentra needs a high-performance backend for real-time monitoring data ingestion and query.

## Decision
Use Rust with Axum web framework and SQLx for database access.

## Consequences
- **Positive:** Zero-cost abstractions, memory safety, excellent async performance
- **Positive:** Compile-time SQL checking via sqlx macros
- **Negative:** Steeper learning curve for new contributors
- **Negative:** Longer compile times
