# KubeCove

KubeCove is a context-first Kubernetes workspace for inspection and guarded troubleshooting.

## Language

**Resource observation**:
The resource evidence shown for a workspace's contexts, including its listed versions and subsequent watched changes. Each context retains its own identity and freshness.

**Shown evidence**:
The resource rows or topology currently visible, including previously loaded evidence retained while a replacement read runs or after that read fails. Shown evidence can be useful while it is stale.

**Freshness**:
Whether shown evidence has settled and its relevant watches are current, without catch-up or a failed reload. Table and ownership-map evidence can have different freshness because the map shows one context.
