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

**Workspace Navigation** means movement within an active workspace scope across resource, GitOps, Helm, incident, settings, and live-session surfaces.
_Avoid_ using it for switching between saved workspaces or changing cluster credentials.

**Port Forward Lifecycle** means starting, restoring, reconnecting, observing, and stopping a guarded Pod or selector-backed Service port-forward session.
_Avoid_ using it for Pod exec or durable background tunnels.

**Pod Exec Lifecycle** means confirming, starting, interacting with, observing, and stopping a guarded exact-Pod exec session.
_Avoid_ using it for local shell execution or owner-backed resource targets.

**Observed RBAC grants** means permissions derived from the RBAC policy objects KubeCove could inspect, with their role and binding provenance.
_Avoid_ presenting them as an API-server authorization verdict or as complete when coverage is partial.

**Permission verification** means an explicit, user-submitted SubjectAccessReview for one identity scenario and one exact resource or non-resource target.
_Avoid_ treating a missing, failed, or no-opinion review as a denial.

**Unknown risk** means KubeCove cannot classify an RBAC object safely because required inventory or referenced policy is unavailable.
_Avoid_ merging Unknown into No flags.

**Kubernetes transport** means direct access to Kubernetes API resources through KubeCove's guarded backend boundary.
_Avoid_ using it for connected provider APIs, CLI execution, or an implied fallback path.

**Connected Argo CD transport** means an explicitly selected Argo CD server connection used for authoritative application inspection, resource comparison, and allowlisted operations.
_Avoid_ treating it as automatic fallback from Kubernetes transport or as an Argo CD CLI session.

**Guarded cluster operation** means a typed, narrowly allowlisted action with an exact visible target, server-backed preview or preflight where available, explicit confirmation, and permission-aware errors.
_Avoid_ using it for generic mutation bridges, arbitrary manifests, or hidden background actions.

**Runtime Secret disclosure** means a transient user-requested reveal of one Secret value during the current UI interaction.
_Avoid_ using it for persisted unredacted state, connected-provider Secret payloads, credentials, tokens, or logs.

**GitOps ownership evidence** means provider metadata or inventory that links a Kubernetes resource to an Argo CD Application, Flux Kustomization, or Flux HelmRelease.
_Avoid_ treating it as a Kubernetes owner reference or proof that every related resource was discovered.

**Argo resource comparison** means connected Argo CD target, live, normalized, or predicted state for one managed resource.
_Avoid_ calling it a selected-resource YAML apply diff, which compares an edited Kubernetes object with its server-side dry-run result.

**Helm Reconciliation** means comparison of decoded Helm release intent with live Kubernetes resource evidence.
_Avoid_ using it for Helm install, upgrade, rollback, uninstall, or CLI execution.
