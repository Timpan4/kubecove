# ADR 0011: Rolling Three-Minor Kubernetes Support

## Status

Accepted.

## Context

KubeCove uses Kubernetes discovery and dynamic resources across clusters with different server versions. A declared compatibility window needs reproducible cluster images and routine coverage; an unbounded promise cannot be verified reliably.

## Decision

KubeCove officially supports the latest three active Kubernetes minor releases for which the repository has digest-pinned Kind images and a green real E2E matrix. The initial window is Kubernetes 1.34, 1.35, and 1.36.

The default real test targets 1.35. Nightly, manual, and release gates run the full 1.34–1.36 matrix. A maintainer PR may advance the window only after:

1. the replacement Kind release publishes digest-pinned node images for all three minors;
2. runner pins and checksums are reviewed;
3. the complete matrix passes; and
4. this ADR and compatibility documentation are updated together.

Older or newer clusters may work, but they are outside the tested support statement.

## Consequences

Compatibility claims match deterministic evidence. Supporting three minors increases scheduled and release CI cost. The window does not move automatically with upstream releases; a maintainer owns each deliberate advance.
