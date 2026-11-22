# Resource pipeline timings

Summary, scope, topology, cache, inspection, metrics, incident, and response
encoding workloads. The parent `resource_catalog.rs` owns prepared inputs and
outputs; each module checks its fixture before entering measurement.

Fixtures and command bridges live in `commands/bench_support/resource_catalog/`.
Keep network, file access, fixture construction, and input reset outside timing.
