# KubeCove Context

## Language

**CRD** means the Kubernetes `CustomResourceDefinition` object that defines a custom API kind.
_Avoid_ using CRD for individual objects shown in resource tables.

**Custom Resources** means CRD-backed API kinds and their instances when KubeCove browses them generically.
_Avoid_ calling this surface Discovered.

**Incident signal** means a read-only resource health, restart, or warning-event cue surfaced for triage.
_Avoid_ using it for durable tickets, alerts, or response lifecycle state.

**Incident Workbench** means the read-only Incident Cockpit workflow that queues signals, guides inspection, and opens existing resource details.
_Avoid_ treating it as a cluster-changing incident response surface.
