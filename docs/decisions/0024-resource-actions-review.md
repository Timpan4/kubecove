# ADR 0024: Selected-resource actions and change review

## Status

Accepted for the requested variant D implementation.

## Decision

The Actions tab uses inline rows, an action filter, visible permission locks, and pending edits collected into one server dry-run and review. Deployment supports replicas, pause/resume, restart, rollback to an owned ReplicaSet revision, container edits, metadata edits, and exact deletion. StatefulSet supports replicas, restart, container and metadata edits, and deletion. DaemonSet supports restart, container and metadata edits, and deletion. Pod and ConfigMap retain exact deletion and gain metadata edits. Dynamic resources and Secret edits remain outside this operation allowlist. The existing selected-resource YAML editor remains the path for other supported manifest edits.

Container edits address regular containers by name and support image, literal environment variable changes, and resource requests/limits. Unchanged environment references and unrelated fields must survive. Metadata edits affect the selected object's labels and annotations, not its Pod template or immutable selector.

Rust owns narrowly typed reads, permission checks, dry-run patches, revision ownership validation, and apply/delete. No arbitrary patch or YAML payload is added to the Actions command. Preview returns current and API-server-produced YAML for a proper diff. Reviewed resource UID and resourceVersion must match before apply. Execution repeats server dry-run and uses optimistic concurrency or delete preconditions. A dry-run error never permits execution. Rollback restores the complete owned revision Pod template, rather than only its image.

Global Settings > Advanced exposes `skipChangeReview`, default false, with an explicit warning before enabling it. It skips the successful review popup only. Server dry-run remains mandatory. Exact deletion always requires confirmation by typing the resource name. Failed checks open the review error state. These rules apply to the Actions tab; selected-resource YAML apply retains ADR 0006's explicit second Apply action.

Permission denial leaves an action visible and locked with its required Kubernetes verb and resource. Failed or indeterminate permission checks do not claim access. The API server remains authoritative, including at execution. Successful actions refresh details, YAML, revisions, and resource lists. Changing target or kubeconfig source invalidates drafts and previews. Independent actions preserve pending drafts; returning from their execution refreshes the baseline before those drafts can be reviewed again.

## Implementation contract

The new command family is `get_resource_action_state`, `preview_resource_action`, and `execute_resource_action`. TypeScript wrappers are `getResourceActionState`, `previewResourceAction`, and `executeResourceAction`. `get_resource_action_state` takes a read-only `ResourceActionStateRequest` extending `ClusterOperationTarget` with optional `kubeconfigEnvVar`; it does not require an action, edits, or confirmation. Preview and execution take `ResourceActionRequest`.

The state command takes `ResourceActionStateRequest`, which extends `ClusterOperationTarget` with optional `kubeconfigEnvVar: string`. Reading state requires no action, edits, or confirmation. Container edit fields other than `name` are optional deltas in TypeScript and default to empty collections when absent in Rust.

`ResourceActionRequest` extends `ClusterOperationTarget` with `action: "edit" | "restart" | "pause" | "resume" | "rollback" | "delete"`, optional `edits: ResourceActionEdits`, optional `revision: number`, optional `expectedUid: string`, optional `expectedResourceVersion: string`, optional `expectedRevisionUid: string`, optional `expectedRevisionResourceVersion: string`, `confirmed: boolean`, and optional `kubeconfigEnvVar: string`. Execution requires both target guard fields from its preview. Rollback execution also requires both revision source guard fields from its preview. The edit request is a single atomic patch for that object.

`ResourceActionEdits` has optional `replicas: number`, `containers: ResourceContainerEdit[]`, `labels: Record<string, string | null>`, and `annotations: Record<string, string | null>`. `ResourceContainerEdit` has `name: string`, optional `image: string`, `env: ResourceEnvEdit[]`, `requests: Record<string, string | null>`, and `limits: Record<string, string | null>`. `ResourceEnvEdit` has `name: string`, optional `value: string`, and optional `remove: boolean`. These maps represent deltas; null removes only the named key. Environment edits preserve untouched references and remove valueFrom only for an explicitly replaced variable.

`ResourceActionState` has `target: ClusterOperationTarget`, `uid: string`, `resourceVersion: string`, optional `replicas: number`, optional `paused: boolean`, `containers: ResourceActionContainer[]`, `labels: Record<string,string>`, `annotations: Record<string,string>`, `permissions: ResourceActionPermission[]`, and `revisions: ResourceActionRevision[]`.

`ResourceActionContainer` has `name: string`, `image: string`, `env: {name:string; value?:string; referenced:boolean}[]`, `requests: Record<string,string>`, and `limits: Record<string,string>`. `ResourceActionPermission` has `action: ResourceActionRequest["action"]`, `allowed: boolean`, `reason: string`, and `requiredPermission: string`. `ResourceActionRevision` has `revision: number` and `images: string[]`. Revision lookup failures must be visible without hiding unrelated actions.

`ResourceActionPreview` has `target: ClusterOperationTarget`, `effect: string`, `beforeYaml: string`, `afterYaml: string`, `uid: string`, `resourceVersion: string`, and optional `revisionUid: string` and `revisionResourceVersion: string`. For rollback, the preview returns the identity of the same owned ReplicaSet object used to build its dry-run patch. Execution re-resolves that selected owned revision and rejects a missing or changed UID/resourceVersion before preparing the patch. This prevents a revision template changed after review from being applied under an unchanged Deployment identity. Requests carry only the selected revision number and narrow source identity values, never a caller-supplied patch or template. Execution returns existing `ClusterOperationResult`. State and preview follow existing resource-display redaction and server-owned metadata cleanup. Secret objects are never returned by this family.

## Verification

Record failure modes before implementation: denied/unknown permissions, invalid or retargeted inputs, stale target/version, unowned revisions, lost environment references or metadata, failed dry-run, mutation before confirmation, delete with review skipped, request completion after changing target, and pending drafts lost by independent actions. Verify those boundaries through focused backend tests and the Actions browser E2E flow. Run required frontend/backend checks and review the final diff before publication.
