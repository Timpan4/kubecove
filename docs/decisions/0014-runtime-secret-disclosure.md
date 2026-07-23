# ADR 0014: Runtime Secret Disclosure

## Status

Accepted.

## Decision

KubeCove persists one global redaction preference. Default remains redact. Per-key secret decoding is transient, explicit, and never changes persisted backend data or logs. Rust redacts connected Argo Secret payloads before crossing Tauri IPC.

## Consequences

Frontend implementation may reveal a selected key only while user requests it. Tokens, kubeconfig credentials, and passwords remain backend-only regardless of preference.
