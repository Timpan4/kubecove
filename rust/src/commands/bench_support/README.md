# Benchmark support

Deterministic fixtures and thin bridges to production command logic, compiled
only with the `bench-support` feature. Resource and domain catalogs keep their
fixtures separate while sharing the registered `backend_hot_paths` target.

Add shared production seams in the owning command module. Keep synthetic data
and benchmark-only orchestration here. See [the catalog](../../../../docs/codspeed-benchmark-catalog.md).
