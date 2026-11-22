# Backend benchmark modules

Supporting modules for the single `backend_hot_paths` Cargo benchmark target.
The resource catalog groups timings by production pipeline. Domain timings live
in the sibling `domain_*.rs` files under `benches/`.

Benchmark function names describe the measured operation and workload because
CodSpeed displays the function name in reports. Do not use generic names such as
`run`. Renaming a benchmark creates a new CodSpeed identity; historical `::run`
results remain under their previous names.

See [the benchmark catalog](../../../docs/codspeed-benchmark-catalog.md) for
caller evidence, fixtures, invariants, and commands.
