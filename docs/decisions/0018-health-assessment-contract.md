# ADR 0018: Health Assessment Contract

## Status

Accepted.

## Decision

Rust evaluates health from typed source evidence and returns one `HealthAssessment` with complete raw evidence, completeness, and every source tied at winning severity.

States are `healthy`, `needsAttention`, `degraded`, `unknown`, and `notEvaluated`. Current sources rank `degraded` above `needsAttention` above `healthy`. Historical or resolved evidence is retained but cannot worsen state; restarts are evidence, not a health state. `unknown` means no current source can classify a recognized health semantic; `notEvaluated` means no health semantic exists.

Unavailable providers make assessment `partial`. Kubernetes fallback still classifies; without fallback result is `unknown`. Argo CD maps `Missing` and `Degraded` to `degraded`, `Progressing` and `OutOfSync` to `needsAttention`, and combined `Healthy` plus `Synced` to `healthy`.

Existing Argo `healthStatus` remains temporarily as raw provider status during consumer migration.
