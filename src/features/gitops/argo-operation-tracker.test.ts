import { afterEach, describe, expect, jest, test } from "bun:test";
import { QueryClient } from "@tanstack/svelte-query";
import { get } from "svelte/store";
import { createMockTauriClient } from "@/lib/tauri-runtime";
import { RESOURCE_RECOVERY_INTERVAL } from "@/lib/resource-watch";
import type {
	ArgoApplicationInspector,
	ArgoOperationRequest,
} from "@/lib/types";
import {
	argoObservation,
	argoOperationOutcome,
	argoOperationProgress,
} from "./argo-operation-progress";
import {
	argoTrackingKey,
	createArgoOperationTracker,
} from "./argo-operation-tracker";

const application = {
	name: "app",
	namespace: "argocd",
	context: "dev",
	workspaceId: "workspace",
	uid: "uid",
	resourceVersion: "old",
};
const request: ArgoOperationRequest = {
	application,
	clusterContext: "dev",
	transport: "kubernetes",
	action: "refresh",
	resources: [],
	resourceVersion: "old",
};
function snapshot(
	patch: Partial<ArgoApplicationInspector> = {},
): ArgoApplicationInspector {
	return {
		application: { ...application, resourceVersion: "current" },
		status: { reconciledAt: "before" },
		history: [],
		resources: [],
		comparisons: [],
		conditions: [],
		operationState: { phase: "Succeeded", startedAt: "old" },
		connected: false,
		transport: "kubernetes",
		provenance: "test",
		...patch,
	};
}
afterEach(() => jest.useRealTimers());

type ReviewFixture = {
	allowed: boolean;
	sessionId: string;
	expiresAt: number;
	reviewedRequest: Partial<ArgoOperationRequest>;
};

describe("Argo operation tracking", () => {
	test("rejects malformed reviews and stops submission when the context closes", async () => {
		let finishReview!: (value: ReviewFixture) => void;
		let reviewStarted: () => void = () => {};

		let review: ReviewFixture | Promise<ReviewFixture> = {
			allowed: true,
			sessionId: "review",
			expiresAt: 1,
			reviewedRequest: {},
		};
		let submissions = 0;
		const queryClient = new QueryClient();
		const tracker = createArgoOperationTracker(
			queryClient,
			createMockTauriClient({
				get_argo_application_inspector: () => snapshot(),
				preflight_argo_operation: () => {
					reviewStarted();
					return review;
				},
				run_argo_operation: () => {
					submissions++;
					return { accepted: true };
				},
				cancel_backend_requests: () => true,
			}),
		);
		const stop = tracker.activate(application);
		try {
			await tracker.run(request);
			expect(get(tracker).get(argoTrackingKey(application))?.phase).toBe(
				"error",
			);
			review = new Promise<ReviewFixture>((resolve) => {
				finishReview = resolve;
			});
			const started = new Promise<void>((resolve) => {
				reviewStarted = resolve;
			});
			const running = tracker.run(request);
			await started;
			stop();
			finishReview({
				allowed: true,
				sessionId: "review",
				expiresAt: 1,
				reviewedRequest: request,
			});
			await running;
			expect(submissions).toBe(0);
		} finally {
			stop();
			queryClient.clear();
		}
	});
	test("blocks repeat refreshes until controller confirmation and survives view unsubscribe", async () => {
		jest.useFakeTimers();
		let current = snapshot();
		const reviews: unknown[] = [];
		let submissions = 0;
		const client = createMockTauriClient({
			get_argo_application_inspector: () => current,
			preflight_argo_operation: (args: { request: ArgoOperationRequest }) => {
				reviews.push(args);
				return {
					allowed: true,
					sessionId: "review",
					expiresAt: 1,
					reviewedRequest: request,
				};
			},
			run_argo_operation: () => {
				submissions++;
				current = snapshot({ refreshRequested: "normal" });
				return { accepted: true };
			},
			cancel_backend_requests: () => true,
		});
		const queryClient = new QueryClient();
		const tracker = createArgoOperationTracker(queryClient, client);
		const stop = tracker.activate(application);
		const unsubscribe = tracker.subscribe(() => {});
		try {
			await Promise.all([
				tracker.run(request),
				tracker.run({ ...request, action: "hardRefresh" }),
			]);
			expect(submissions).toBe(1);
			expect(reviews).toEqual([
				{
					request: {
						...request,
						application: { ...application, resourceVersion: "current" },
						resourceVersion: "current",
					},
				},
			]);
			expect(get(tracker).get(argoTrackingKey(application))?.busy).toBe(true);
			unsubscribe();
			current = snapshot({ status: { reconciledAt: "after" } });
			const completed = new Promise<void>((resolve) => {
				const stopWatching = tracker.subscribe((states) => {
					if (states.get(argoTrackingKey(application))?.phase === "succeeded") {
						stopWatching();
						resolve();
					}
				});
			});
			jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
			await completed;
			expect(get(tracker).get(argoTrackingKey(application))?.phase).toBe(
				"succeeded",
			);
			expect(submissions).toBe(1);
		} finally {
			stop();
			queryClient.clear();
		}
	});
	test("read failure after acceptance stays unknown and retries observation without resubmitting", async () => {
		let accepted = false;
		let fail = true;
		let submissions = 0;
		const client = createMockTauriClient({
			get_argo_application_inspector: () => {
				if (accepted && fail)
					throw { kind: "network", message: "connection reset" };
				return snapshot(accepted ? { status: { reconciledAt: "after" } } : {});
			},
			preflight_argo_operation: () => ({
				allowed: true,
				sessionId: "review",
				expiresAt: 1,
				reviewedRequest: request,
			}),
			run_argo_operation: () => {
				accepted = true;
				submissions++;
				return { accepted: true };
			},
			cancel_backend_requests: () => true,
		});
		const queryClient = new QueryClient();
		const tracker = createArgoOperationTracker(queryClient, client);
		const stop = tracker.activate(application);
		try {
			await tracker.run(request);
			expect(get(tracker).get(argoTrackingKey(application))).toMatchObject({
				phase: "unknown",
				busy: true,
				error: "connection reset",
			});
			await tracker.run(request);
			fail = false;
			await tracker.observe(argoTrackingKey(application));
			expect(submissions).toBe(1);
			expect(get(tracker).get(argoTrackingKey(application))?.phase).toBe(
				"succeeded",
			);
		} finally {
			stop();
			queryClient.clear();
		}
	});
	test("renders serialized backend errors and does not submit after rejected review", async () => {
		let submissions = 0;
		const client = createMockTauriClient({
			get_argo_application_inspector: () => snapshot(),
			preflight_argo_operation: () => {
				throw { kind: "forbidden", message: "applications patch forbidden" };
			},
			run_argo_operation: () => {
				submissions++;
				return { accepted: true };
			},
			cancel_backend_requests: () => true,
		});
		const queryClient = new QueryClient();
		const tracker = createArgoOperationTracker(queryClient, client);
		const stop = tracker.activate(application);
		try {
			await tracker.run(request);
			expect(get(tracker).get(argoTrackingKey(application))).toMatchObject({
				phase: "error",
				busy: false,
				error: "applications patch forbidden",
			});
			expect(submissions).toBe(0);
		} finally {
			stop();
			queryClient.clear();
		}
	});
	test("old successful sync state cannot complete a newly accepted sync", () => {
		const before = snapshot();
		expect(
			argoOperationOutcome("sync", argoObservation(before), before, false),
		).toBe("pending");
		expect(
			argoOperationOutcome(
				"sync",
				argoObservation(before),
				snapshot({
					operationState: {
						phase: "Succeeded",
						startedAt: "old",
						message: "updated old message",
					},
				}),
				false,
			),
		).toBe("pending");
		const running = snapshot({
			operationRequested: true,
			operationState: { phase: "Running", startedAt: "new" },
		});
		expect(
			argoOperationOutcome("sync", argoObservation(before), running, false),
		).toBe("pending");
		const failed = snapshot({
			operationState: {
				phase: "Failed",
				startedAt: "new",
				message: "hook failed",
				syncResult: {
					resources: [
						{
							kind: "Job",
							name: "migrate",
							hookPhase: "Failed",
							message: "migration failed",
						},
					],
				},
			},
		});
		expect(
			argoOperationOutcome("sync", argoObservation(before), failed, true),
		).toBe("failed");
		expect(argoOperationProgress(failed).resources).toEqual([
			{
				kind: "Job",
				name: "migrate",
				namespace: null,
				phase: "Failed",
				message: "migration failed",
			},
		]);
	});
});
