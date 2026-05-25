# ADR 003: TimescaleDB for Time-Series Metrics

## Status: Accepted
## Date: 2025-01-01

## Context
Metrics data is time-series and needs efficient storage, aggregation, and retention management.

## Decision
Use TimescaleDB (PostgreSQL extension) with hypertables, continuous aggregates, and retention policies.

## Consequences
- **Positive:** SQL-compatible, no separate time-series database to manage
- **Positive:** Continuous aggregates for dashboard performance
- **Positive:** Built-in compression and retention policies
- **Negative:** Requires TimescaleDB license for some features
- **Negative:** Single-node limitation without TimescaleDB Cloud
