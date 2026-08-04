import type { ArgoOperationRequest } from "@/lib/gitops-types";
import { runArgoOperationLifecycle } from "./argo-operation-lifecycle";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void | Promise<void>): void;
declare function expect<T>(actual: T): {
	toEqual(expected: unknown): void;
};

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (error: unknown) => void;
	const promise = new Promise<T>((next, fail) => {
		resolve = next;
		reject = fail;
	});
	return { promise, resolve, reject };
}

const request: ArgoOperationRequest = {
	transport: "kubernetes",
	application: { name: "guestbook", namespace: "argocd" },
	action: "refresh",
	resources: [],
};

function allowedPreflight() {
	return {
		allowed: true,
		sessionId: "session-1",
		expiresAt: 1,
		reviewedRequest: request,
	};
}

async function runWithScopeChange(boundary: "preflight" | "run" | "refresh") {
	const preflight = deferred<ReturnType<typeof allowedPreflight>>();
	const run = deferred<{ accepted: boolean }>();
	const refresh = deferred<void>();
	const preflightStarted = deferred<void>();
	const runStarted = deferred<void>();
	const refreshStarted = deferred<void>();
	const phases: string[] = [];
	const calls: string[] = [];
	let current = true;
	const lifecycle = runArgoOperationLifecycle({
		request,
		preflight: async () => {
			calls.push("preflight");
			preflightStarted.resolve();
			return preflight.promise;
		},
		run: async () => {
			calls.push("run");
			runStarted.resolve();
			return run.promise;
		},
		refresh: async () => {
			calls.push("refresh:captured-application-a");
			refreshStarted.resolve();
			return refresh.promise;
		},
		isCurrent: () => current,
		onPhase: (phase) => phases.push(phase),
	});

	await preflightStarted.promise;
	if (boundary === "preflight") current = false;
	preflight.resolve(allowedPreflight());
	await runStarted.promise;
	if (boundary === "run") current = false;
	run.resolve({ accepted: true });
	await refreshStarted.promise;
	if (boundary === "refresh") current = false;
	refresh.resolve();
	await lifecycle;
	return { phases, calls };
}

describe("Argo operation lifecycle", () => {
	test("refreshes captured work without stale callbacks after each scope-change boundary", async () => {
		for (const [boundary, phases] of [
			["preflight", ["authorizing"]],
			["run", ["authorizing", "submitting"]],
			["refresh", ["authorizing", "submitting", "refreshing"]],
		] as const) {
			const result = await runWithScopeChange(boundary);
			expect(result.calls).toEqual(["preflight", "run", "refresh:captured-application-a"]);
			expect(result.phases).toEqual(phases);
		}
	});

	test("suppresses stale errors", async () => {
		const preflight = deferred<ReturnType<typeof allowedPreflight>>();
		const phases: string[] = [];
		let current = true;
		const lifecycle = runArgoOperationLifecycle({
			request,
			preflight: () => preflight.promise,
			run: async () => ({ accepted: true }),
			refresh: async () => {},
			isCurrent: () => current,
			onPhase: (phase) => phases.push(phase),
		});
		current = false;
		preflight.reject(new Error("preflight failed"));
		await lifecycle.catch(() => {});

		expect(phases).toEqual(["authorizing"]);
	});
});
