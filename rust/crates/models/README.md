# Models

Frontend-safe serde contracts, shared health assessment, endpoint validation, and typed backend errors. Existing Kubernetes error conversions stay with `AppError` to preserve source-chain classification and Rust's orphan rules. Internal cancellation markers and endpoint normalization are Rust APIs, not additional frontend commands.

This foundational crate must not depend on domain commands, runtime state, or hosts. Serialization and error-classification tests live beside their contracts.
