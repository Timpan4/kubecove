# Live sessions

Owned resource/event watches, Pod/aggregated logs, Pod exec, Pod/selector-backed Service port forwards, and scope cleanup. Existing registries remain responsible for admission, cancellation and termination.

Depends on models and Kubernetes infrastructure. Argo service tunnels and the headless server reuse the forwarding/target-resolution APIs; this crate must not depend on GitOps or the host. `bench-support` exposes existing deterministic adapters, not new runtime commands.
