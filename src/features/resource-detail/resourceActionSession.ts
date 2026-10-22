import type { QueryClient } from "@tanstack/svelte-query";
import { queryMatchers } from "@/lib/queryKeys";
import {
	executeResourceAction,
	getResourceActionState,
	previewResourceAction,
	type TauriClient,
} from "@/lib/tauri";
import type {
	ResourceActionEdits,
	ResourceActionPreview,
	ResourceActionRequest,
	ResourceActionState,
	ResourceSummary,
} from "@/lib/types";
import { getErrorMessage } from "./helpers";
import {
	actionAllowed,
	actionTarget,
	type ResourceActionId,
	resourceActionRows,
} from "./resource-actions-model";
import { type DraftChange, hasPendingEdits } from "./resourceActionDrafts";

// ADR 0024 review session for one selected resource: drafts, server dry-run
// preview, guarded execution, and the refresh that follows. Every completion
// checks that its load or review is still the current one, so results never
// land on a target the user has left.

export interface ResourceActionTarget {
	resource: ResourceSummary;
	kubeconfigSourceKey?: string;
}

export type ResourceActionReviewStatus = "checking" | "ready" | "executing" | "failed";

export interface ResourceActionReview {
	action: ResourceActionId;
	/** The exact resource name; deletion is confirmed by typing it. */
	name: string;
	status: ResourceActionReviewStatus;
	/** False while a skipped review runs inline. */
	open: boolean;
	/** Settings skip the successful review dialog for this action. */
	skipRequested: boolean;
	preview: ResourceActionPreview | null;
	error: string | null;
	/** The API server may have received the failed execution. */
	executionAttempted: boolean;
	confirmation: string;
	canExecute: boolean;
}

export interface ResourceActionSessionState {
	actionState: ResourceActionState | null;
	loading: boolean;
	loadError: string | null;
	edits: ResourceActionEdits;
	review: ResourceActionReview | null;
	/** Progress of a skipped review, shown inline instead of a dialog. */
	inlineStatus: string | null;
	/** Effect of the last successful action. */
	result: string | null;
}

export interface ResourceActionSessionOptions {
	client: TauriClient;
	queryClient: QueryClient;
	/** Read when a review starts. */
	skipReview: () => boolean;
	/** An edit action applied the drafts it reviewed. */
	onDraftsApplied?: () => void;
}

export interface ResourceActionSession {
	readonly current: ResourceActionSessionState;
	/** Calls the listener now and after every change. */
	subscribe(listener: (state: ResourceActionSessionState) => void): () => void;
	/** Returns true when the target or kubeconfig source changed, which drops drafts and reviews. */
	retarget(target: ResourceActionTarget): boolean;
	retry(): void;
	/** Applies a draft change against the loaded baseline; false when refused. */
	draft(change: DraftChange): boolean;
	review(action: ResourceActionId, revision?: number): Promise<void>;
	confirm(name: string): void;
	execute(): Promise<void>;
	closeReview(): void;
}

interface ReviewRun {
	target: ResourceActionTarget;
	request: ResourceActionRequest;
	view: ResourceActionReview;
}

const IDLE: ResourceActionSessionState = {
	actionState: null,
	loading: false,
	loadError: null,
	edits: {},
	review: null,
	inlineStatus: null,
	result: null,
};

function canExecute(view: ResourceActionReview): boolean {
	return (
		view.status === "ready" &&
		view.preview !== null &&
		view.error === null &&
		(view.action !== "delete" || view.confirmation === view.name)
	);
}

function executionRequest(request: ResourceActionRequest, preview: ResourceActionPreview): ResourceActionRequest {
	const guarded: ResourceActionRequest = {
		...request,
		expectedUid: preview.uid,
		expectedResourceVersion: preview.resourceVersion,
		confirmed: true,
	};
	if (request.action === "rollback") {
		guarded.expectedRevisionUid = preview.revisionUid;
		guarded.expectedRevisionResourceVersion = preview.revisionResourceVersion;
	}
	return guarded;
}

function targetIdentity({ resource, kubeconfigSourceKey }: ResourceActionTarget): string {
	return JSON.stringify([actionTarget(resource), kubeconfigSourceKey ?? ""]);
}

export function createResourceActionSession(options: ResourceActionSessionOptions): ResourceActionSession {
	const { client, queryClient } = options;
	const listeners = new Set<(state: ResourceActionSessionState) => void>();
	let state = IDLE;
	let target: ResourceActionTarget | null = null;
	let identity = "";
	let load: symbol | null = null;
	let run: ReviewRun | null = null;

	function publish(patch: Partial<ResourceActionSessionState>) {
		state = { ...state, ...patch };
		for (const listener of listeners) listener(state);
	}

	function updateRun(current: ReviewRun, view: Partial<ResourceActionReview>, patch: Partial<ResourceActionSessionState> = {}) {
		const next = { ...current.view, ...view };
		current.view = { ...next, canExecute: canExecute(next) };
		publish({ ...patch, review: current.view });
	}

	async function loadBaseline(from: ResourceActionTarget) {
		const token = Symbol("action-state");
		load = token;
		try {
			const actionState = await getResourceActionState(client, {
				...actionTarget(from.resource),
				kubeconfigEnvVar: from.kubeconfigSourceKey,
			});
			if (load === token) publish({ actionState, loadError: null, loading: false });
		} catch (cause) {
			if (load === token) publish({ actionState: null, loadError: getErrorMessage(cause), loading: false });
		}
	}

	async function execute() {
		const current = run;
		const preview = current?.view.preview;
		if (!current || !preview || !current.view.canExecute) return;
		updateRun(current, { status: "executing", error: null });
		let effect: string;
		try {
			effect = (await executeResourceAction(client, executionRequest(current.request, preview))).effect;
		} catch (cause) {
			if (run !== current) return;
			updateRun(
				current,
				{ status: "failed", error: getErrorMessage(cause), executionAttempted: true, open: true },
				{ inlineStatus: null },
			);
			return;
		}
		// The change happened, so its reads refresh even when the view has moved on.
		await queryClient.invalidateQueries({
			predicate: queryMatchers.resourceAction(current.target.resource, current.target.kubeconfigSourceKey),
		});
		if (run !== current) return;
		run = null;
		// Drafts edited while this one executed stay pending; independent actions keep every draft.
		const applied =
			current.request.action === "edit" && JSON.stringify(state.edits) === JSON.stringify(current.request.edits);
		publish({ review: null, inlineStatus: null, result: effect, edits: applied ? {} : state.edits });
		if (applied) options.onDraftsApplied?.();
		await loadBaseline(current.target);
	}

	return {
		get current() {
			return state;
		},
		subscribe(listener) {
			listeners.add(listener);
			listener(state);
			return () => {
				listeners.delete(listener);
			};
		},
		retarget(next) {
			const nextIdentity = targetIdentity(next);
			target = next;
			if (nextIdentity === identity) return false;
			identity = nextIdentity;
			run = null;
			load = null;
			const actionable = resourceActionRows(next.resource).length > 0;
			state = { ...IDLE, loading: actionable };
			publish({});
			if (actionable) void loadBaseline(next);
			return true;
		},
		retry() {
			if (!target || resourceActionRows(target.resource).length === 0) return;
			publish({ loading: true, loadError: null });
			void loadBaseline(target);
		},
		draft(change) {
			if (!state.actionState) return false;
			const edits = change(state.edits, state.actionState);
			if (edits === state.edits) return false;
			publish({ edits });
			return true;
		},
		async review(action, revision) {
			if (!target || !actionAllowed(state.actionState, action)) return;
			if (action === "edit" && !hasPendingEdits(state.edits)) return;
			// A running execution keeps its result and refresh.
			if (run?.view.status === "executing") return;
			const skipRequested = action !== "delete" && options.skipReview();
			const request: ResourceActionRequest = {
				...actionTarget(target.resource),
				action,
				confirmed: false,
				kubeconfigEnvVar: target.kubeconfigSourceKey,
			};
			if (action === "edit") request.edits = structuredClone(state.edits);
			if (revision !== undefined) request.revision = revision;
			const view: ResourceActionReview = {
				action,
				name: request.name,
				status: "checking",
				open: !skipRequested,
				skipRequested,
				preview: null,
				error: null,
				executionAttempted: false,
				confirmation: "",
				canExecute: false,
			};
			const current: ReviewRun = { target, request, view };
			run = current;
			publish({
				review: view,
				result: null,
				inlineStatus: skipRequested ? "Checking with the API server…" : null,
			});
			let preview: ResourceActionPreview;
			try {
				preview = await previewResourceAction(client, request);
			} catch (cause) {
				// A failed dry run opens the review with its error and never executes.
				if (run === current) updateRun(current, { status: "failed", error: getErrorMessage(cause), open: true }, { inlineStatus: null });
				return;
			}
			if (run !== current) return;
			if (skipRequested && preview.beforeYaml !== preview.afterYaml) {
				updateRun(current, { status: "ready", preview }, { inlineStatus: "Applying the reviewed action…" });
				await execute();
				return;
			}
			updateRun(current, { status: "ready", preview, open: true }, { inlineStatus: null });
		},
		confirm(name) {
			if (run) updateRun(run, { confirmation: name });
		},
		execute,
		closeReview() {
			if (!run || run.view.status === "executing") return;
			run = null;
			publish({ review: null, inlineStatus: null });
		},
	};
}
