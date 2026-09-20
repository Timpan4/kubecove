import type { QueryClient } from "@tanstack/svelte-query";
import { writable } from "svelte/store";
import { messageFromError } from "@/lib/error-redaction";
import { createFiniteReadRequest } from "@/lib/finite-read-lifecycle";
import { queryKeys } from "@/lib/queryKeys";
import { RESOURCE_RECOVERY_INTERVAL } from "@/lib/resource-watch";
import {
	cancelBackendRequests,
	createTauriClient,
	getArgoApplicationInspector,
	preflightArgoOperation,
	runArgoOperation,
	type TauriClient,
} from "@/lib/tauri";
import type { ArgoApplicationRef, ArgoOperationRequest } from "@/lib/types";
import {
	argoObservation,
	argoOperationOutcome,
	argoOperationProgress,
} from "./argo-operation-progress";

export function argoTrackingKey(
	app: ArgoApplicationRef,
	source?: string | null,
): string {
	return JSON.stringify([
		source ?? "",
		app.context,
		app.workspaceId,
		app.namespace,
		app.name,
	]);
}

export interface TrackedArgoOperation {
	request: ArgoOperationRequest;
	phase:
		| "authorizing"
		| "submitting"
		| "pending"
		| "unknown"
		| "succeeded"
		| "failed"
		| "error";
	busy: boolean;
	message: string;
	error: string | null;
	progress?: ReturnType<typeof argoOperationProgress>;
}

interface OperationJob {
	state: TrackedArgoOperation;
	baseline?: ReturnType<typeof argoObservation>;
	sawPending: boolean;
	reading: boolean;
	cancelScope: string;
}

export function createArgoOperationTracker(
	queryClient: QueryClient,
	client: TauriClient = createTauriClient(),
) {
	const states = writable<ReadonlyMap<string, TrackedArgoOperation>>(new Map());
	const jobs = new Map<string, OperationJob>();
	let activeScope: string | null = null;
	let timer: ReturnType<typeof setTimeout> | undefined;
	const scope = (app: ArgoApplicationRef, source?: string | null) =>
		JSON.stringify([source ?? "", app.context, app.workspaceId]);
	function current(key: string, job: OperationJob) {
		return (
			jobs.get(key) === job &&
			activeScope ===
				scope(job.state.request.application, job.state.request.kubeconfigEnvVar)
		);
	}
	function publish() {
		states.set(new Map([...jobs].map(([key, job]) => [key, { ...job.state }])));
	}
	function read(job: OperationJob) {
		const request = job.state.request;
		return getArgoApplicationInspector(
			client,
			{
				clusterContext:
					request.clusterContext ?? request.application.context ?? "",
				kubeconfigEnvVar: request.kubeconfigEnvVar ?? undefined,
				connectionId: request.connectionId ?? undefined,
				transport: request.transport,
				application: request.application,
				redactSecrets: true,
			},
			createFiniteReadRequest(job.cancelScope, "argo-operation-observation"),
		);
	}
	async function observe(key: string) {
		const job = jobs.get(key);
		if (!job?.baseline || !job.state.busy || job.reading || !current(key, job))
			return;
		job.reading = true;
		try {
			const inspector = await read(job);
			if (!current(key, job)) return;
			if (job.baseline.uid && inspector.application.uid !== job.baseline.uid) {
				job.state = {
					...job.state,
					phase: "error",
					busy: false,
					error:
						"Application was replaced. The previous operation's outcome is unknown.",
					message: "Application identity changed",
				};
				publish();
				return;
			}
			const progress = argoOperationProgress(inspector);
			const outcome = argoOperationOutcome(
				job.state.request.action,
				job.baseline,
				inspector,
				job.sawPending,
			);
			job.sawPending ||=
				job.state.request.action === "refresh" ||
				job.state.request.action === "hardRefresh"
					? progress.refreshing
					: progress.active;
			job.state = {
				...job.state,
				progress,
				busy: outcome === "pending",
				phase: outcome,
				error:
					outcome === "failed"
						? progress.message || "Argo CD reported operation failure."
						: null,
				message:
					outcome === "pending"
						? "Accepted; waiting for Argo CD to complete"
						: outcome === "succeeded"
							? "Argo CD confirmed completion"
							: "Argo CD reported failure",
			};
			const app = job.state.request.application;
			void queryClient.invalidateQueries({
				queryKey: queryKeys.argoWorkspaceApplicationScope(
					app.context ?? "",
					app.workspaceId ?? "",
					app.name,
					app.namespace,
					job.state.request.kubeconfigEnvVar ?? undefined,
				),
			});
			void queryClient.invalidateQueries({
				queryKey: queryKeys.argoApps(
					app.context ?? "",
					job.state.request.kubeconfigEnvVar ?? undefined,
				),
			});
		} catch (error) {
			if (!current(key, job)) return;
			job.state = {
				...job.state,
				phase: "unknown",
				error: messageFromError(error),
				message:
					"Operation accepted; current status unavailable. Retrying observation.",
			};
		} finally {
			job.reading = false;
			if (current(key, job)) publish();
		}
	}
	async function run(request: ArgoOperationRequest) {
		const key = argoTrackingKey(request.application, request.kubeconfigEnvVar);
		if (
			jobs.get(key)?.state.busy ||
			activeScope !== scope(request.application, request.kubeconfigEnvVar)
		)
			return;
		const job: OperationJob = {
			state: {
				request,
				phase: "authorizing",
				busy: true,
				message: "Loading current Application and checking authorization…",
				error: null,
			},
			sawPending: false,
			reading: false,
			cancelScope: `argo-operation:${key}`,
		};
		jobs.set(key, job);
		publish();
		try {
			const before = await read(job);
			if (!current(key, job)) return;
			if (
				request.application.uid &&
				before.application.uid !== request.application.uid
			)
				throw new Error(
					"Application was replaced. Reload before submitting an operation.",
				);
			const progress = argoOperationProgress(before);
			if (before.refreshRequested || progress.active)
				throw new Error(
					"This Application already has an operation in progress. Wait for it to finish.",
				);
			job.baseline = argoObservation(before);
			job.state.request = {
				...request,
				application: {
					...request.application,
					...before.application,
					context: request.application.context,
					workspaceId: request.application.workspaceId,
				},
				resourceVersion: before.application.resourceVersion,
			};
			const review = await preflightArgoOperation(client, job.state.request);
			if (!current(key, job)) return;
			if (
				review.allowed !== true ||
				!review.sessionId ||
				String(review.sessionId) !== review.sessionId ||
				Number(review.expiresAt) !== review.expiresAt ||
				!["connected", "kubernetes"].includes(
					review.reviewedRequest?.transport ?? "",
				) ||
				String(review.reviewedRequest?.application?.name) !==
					review.reviewedRequest?.application?.name ||
				!Array.isArray(review.reviewedRequest?.resources)
			)
				throw new Error(review.reason ?? "Operation unavailable");
			job.state = {
				...job.state,
				phase: "submitting",
				message: "Submitting reviewed operation…",
			};
			publish();
			const result = await runArgoOperation(client, {
				sessionId: review.sessionId,
				confirmation: review.sessionId,
			});
			if (!current(key, job)) return;
			if (result.accepted !== true)
				throw new Error(result.message || "Operation rejected");
			job.state = {
				...job.state,
				phase: "pending",
				message: "Operation accepted; waiting for Argo CD",
			};
			publish();
			await observe(key);
		} catch (error) {
			if (!current(key, job)) return;
			job.state = {
				...job.state,
				phase: "error",
				busy: false,
				error: messageFromError(error),
				message:
					"Operation could not be confirmed. Reload state before retrying.",
			};
			publish();
		}
	}
	function activate(
		appScope: Pick<ArgoApplicationRef, "context" | "workspaceId">,
		source?: string,
	) {
		activeScope = JSON.stringify([
			source ?? "",
			appScope.context,
			appScope.workspaceId,
		]);
		function tick() {
			for (const [key, job] of jobs)
				if (["pending", "unknown"].includes(job.state.phase)) void observe(key);
			timer = setTimeout(tick, RESOURCE_RECOVERY_INTERVAL);
		}
		timer = setTimeout(tick, RESOURCE_RECOVERY_INTERVAL);
		return () => {
			activeScope = null;
			if (timer) clearTimeout(timer);
			for (const job of jobs.values())
				void cancelBackendRequests(client, job.cancelScope).catch(() => {});
			jobs.clear();
			publish();
		};
	}
	return { subscribe: states.subscribe, run, observe, activate };
}

const trackers = new WeakMap<
	QueryClient,
	ReturnType<typeof createArgoOperationTracker>
>();
export function getArgoOperationTracker(queryClient: QueryClient) {
	let tracker = trackers.get(queryClient);
	if (!tracker) {
		tracker = createArgoOperationTracker(queryClient);
		trackers.set(queryClient, tracker);
	}
	return tracker;
}
