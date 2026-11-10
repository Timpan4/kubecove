import { expect, test } from "bun:test";
import { isCancelledError, QueryClient } from "@tanstack/svelte-query";
import { cancelledQueryRetry } from "./query-retry";

// Failure modes are recorded in gpui/target/retry-fix-report.md before implementation.
test("explicit opt-outs retry only backend cancellation within the existing bound", () => {
    for (const kind of ["network", "cluster", "forbidden", "validation", "credentialUnavailable"]) {
        expect(cancelledQueryRetry(0, { kind, message: "request cancelled" })).toBe(false);
    }
    expect(cancelledQueryRetry(0, new DOMException("Aborted", "AbortError"))).toBe(false);
    expect(cancelledQueryRetry(0, { kind: "cancelled" })).toBe(true);
    expect(cancelledQueryRetry(1, { kind: "cancelled" })).toBe(true);
    expect(cancelledQueryRetry(2, { kind: "cancelled" })).toBe(false);
});

test("frontend cancellation stops the retryer before backend workspace cancellation rejects", async () => {
    const client = new QueryClient();
    let rejectRead: (cause: { kind: string }) => void = () => {};
    let calls = 0;
    const result = client.fetchQuery({
        queryKey: ["cancelled-read"],
        queryFn: () => {
            calls++;
            return new Promise<string>((_, reject) => { rejectRead = reject; });
        },
        retry: cancelledQueryRetry,
    }).catch((error) => error);
    await client.cancelQueries();
    rejectRead({ kind: "cancelled" });
    expect(isCancelledError(await result)).toBe(true);
    expect(calls).toBe(1);
    client.clear();
});
