# Browser Dev Mock Checklist

Captured from the Chrome sweep of browser-only mock mode on 2026-06-29.

## Mock Gaps

- [x] Make ownership map data attach ConfigMaps and Secrets more like the real cluster instead of leaving them mostly ownerless or statically grouped.
- [x] Make network flow relationships less synthetic so Service, Ingress, EndpointSlice, and Pod wiring matches real cluster shape more closely.
- [x] Add richer RBAC mock data: roles, cluster roles, bindings, and more than one service account path.
- [x] Add believable port-forward and exec session states so the live-session surfaces are useful in browser mock mode.
- [x] Add more GitOps breadth: multiple Argo apps, app sets, projects, Flux resources, and Helm releases.
- [x] Make compare mode show two meaningfully different cluster states instead of one rich cluster and one mostly empty lane.
- [x] Give Config, Storage, and Discovered their own mock stories instead of feeling like filtered views of the same resource list.
- [x] Vary incident data between clusters so the cockpit feels like a real comparison, not duplicated alerts.

## Missing Or Thin Behavior

- [ ] Surface real-looking empty states for pages that have no active data, especially Port Forwards and exec.
- [ ] Expand resource detail coverage for YAML, events, logs, metrics, and topology on representative resources.
- [x] Make the GitOps detail actions land on a fuller inspection flow instead of stopping at the summary card.
- [x] Reduce the visual repetition between mock-dev and docker-desktop in overview and incident surfaces.

## Follow-Up

- [ ] Re-run the Chrome sweep after the next mock-data pass and remove any items that no longer reproduce.
