import { expect, test } from "bun:test";
import { queryRetry } from "./query-retry";

// Auth failures must not become repeated plugin launches through query retries.
test("credential failures wait for an explicit retry", () => {
	for (const message of ["Plugin exited with status 1", "connection refused", ""]) {
		expect(queryRetry(0, { kind: "credentialUnavailable", message })).toBe(false);
	}
	expect(queryRetry(0, { kind: "network", message: "connection refused" })).toBe(true);
});
