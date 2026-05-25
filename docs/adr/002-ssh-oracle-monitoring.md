# ADR 002: SSH + sqlplus for Oracle Monitoring

## Status: Accepted (under review for migration to rust-oracle)
## Date: 2025-01-01

## Context
Oracle monitoring requires executing SQL against remote Oracle instances.

## Decision
Use SSH to remote hosts and pipe SQL to sqlplus via local Oracle Instant Client.

## Consequences
- **Positive:** Works without Oracle client libraries on the Zentra server (only on target)
- **Positive:** Familiar to Oracle DBAs
- **Negative:** Command injection risk (mitigated in Phase 1 with base64 encoding)
- **Negative:** Performance overhead of SSH + process spawn per query
- **Negative:** Requires SSH key distribution to all Oracle hosts

## Future
Evaluate `rust-oracle` crate for native JDBC-like connections.
