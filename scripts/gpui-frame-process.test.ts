import { expect, test } from "bun:test";
import { isParityProcess } from "./gpui-frame-process";

test("detects the executing Bun parity script and native test binary", () => {
	expect(isParityProcess("/usr/bin/bun", ["bun", "scripts/e2e-parity.ts"])).toBe(true);
	expect(isParityProcess("/usr/bin/bun", ["bun", "run", "/checkout/scripts/e2e-parity.ts"])).toBe(true);
	expect(isParityProcess("/checkout/target/debug/deps/parity-ab1234", ["/checkout/target/debug/deps/parity-ab1234"])).toBe(true);
});

test("ignores shell observers, timeout, tail and lock waiters mentioning parity", () => {
	for (const executable of ["zsh", "timeout", "tail", "flock"]) {
		expect(isParityProcess(`/usr/bin/${executable}`, [executable, "bun scripts/e2e-parity.ts", "parity-ab1234"])).toBe(false);
	}
	expect(isParityProcess("/usr/bin/flock", ["flock", "heavy.lock", "bun", "scripts/e2e-parity.ts"])).toBe(false);
});

test("ignores unrelated Bun scripts and script names embedded in arguments", () => {
	expect(isParityProcess("/usr/bin/bun", ["bun", "observer.ts", "scripts/e2e-parity.ts"])).toBe(false);
	expect(isParityProcess("/usr/bin/bun", ["bun", "scripts/gpui-frames.ts"])).toBe(false);
	expect(isParityProcess("/usr/bin/bun", ["bun", "-e", "console.log('scripts/e2e-parity.ts')"])).toBe(false);
});
