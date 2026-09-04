import { expect, test } from "bun:test";
import { isLinuxAllocatorFailure } from "../scripts/lint-anti-slop";

test("allocator fallback only handles the known Linux crash", () => {
	const panic = "thread 'tokio-rt-worker' panicked at crates/oxc_allocator/src/pool/fixed_size.rs:112:67";
	expect(isLinuxAllocatorFailure("linux", panic)).toBe(true);
	expect(isLinuxAllocatorFailure("win32", panic)).toBe(false);
	expect(isLinuxAllocatorFailure("darwin", panic)).toBe(false);
	expect(isLinuxAllocatorFailure("linux", "error anti-slop(no-runtime-typeof)")).toBe(false);
	expect(isLinuxAllocatorFailure("linux", "panicked at another/file.rs")).toBe(false);
});
