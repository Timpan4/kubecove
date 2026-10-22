import { afterEach, expect, jest, test } from "bun:test";
import {
	createWatchRefreshThrottle,
	watchKeyMatches,
} from "./resource-watch-refresh";

afterEach(() => jest.useRealTimers());

test("focused Argo watches that specify apiVersion match discovered kinds with an explicit group", () => {
	const focused = {
		resourceKind: { kind: "Application", apiVersion: "argoproj.io/v1alpha1" },
		namespace: "argocd",
	};
	const discovered = {
		resourceKind: {
			kind: "Application",
			group: "argoproj.io",
			apiVersion: "argoproj.io/v1alpha1",
		},
		namespace: "argocd",
	};
	expect(watchKeyMatches(focused, discovered)).toBe(true);
	expect(watchKeyMatches(discovered, focused)).toBe(true);
	expect(
		watchKeyMatches(focused, {
			...discovered,
			resourceKind: { ...discovered.resourceKind, group: "other.io" },
		}),
	).toBe(false);
	expect(watchKeyMatches(focused, { ...discovered, namespace: "other" })).toBe(
		false,
	);
});
// Failure modes: trailing updates get lost, continuous churn postpones forever,
// overlapping requests accumulate, failures wedge the throttle, disposal refetches old scope.
test("watch bursts refresh immediately and retain the last event at the throttle boundary", async () => {
	jest.useFakeTimers();
	let calls = 0;
	const throttle = createWatchRefreshThrottle(async () => {
		calls++;
	});
	throttle.notify();
	expect(calls).toBe(1);
	await Promise.resolve();
	throttle.notify();
	jest.advanceTimersByTime(249);
	expect(calls).toBe(1);
	jest.advanceTimersByTime(1);
	expect(calls).toBe(2);
	await Promise.resolve();
	jest.advanceTimersByTime(250);
	expect(calls).toBe(2);
	throttle.dispose();
});

test("events during an in-flight refresh produce one trailing refresh without overlap", async () => {
	jest.useFakeTimers();
	let calls = 0;
	let finish!: () => void;
	const throttle = createWatchRefreshThrottle(() => {
		calls++;
		return new Promise<void>((resolve) => {
			finish = resolve;
		});
	});
	throttle.notify();
	throttle.notify();
	jest.advanceTimersByTime(2000);
	throttle.notify();
	expect(calls).toBe(1);
	finish();
	await Promise.resolve();
	expect(calls).toBe(2);
	throttle.notify();
	throttle.dispose();
	finish();
	await Promise.resolve();
	jest.advanceTimersByTime(2000);
	expect(calls).toBe(2);
});

test("a rejected refresh allows subsequent events and disposal cancels a queued trailing call", async () => {
	jest.useFakeTimers();
	let calls = 0;
	const throttle = createWatchRefreshThrottle(async () => {
		calls++;
		throw new Error("offline");
	});
	throttle.notify();
	await Promise.resolve();
	throttle.notify();
	jest.advanceTimersByTime(250);
	expect(calls).toBe(2);
	await Promise.resolve();
	throttle.notify();
	throttle.dispose();
	jest.advanceTimersByTime(250);
	expect(calls).toBe(2);
});
