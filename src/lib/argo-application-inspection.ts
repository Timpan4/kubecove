import { QueryObserver, type QueryClient } from "@tanstack/svelte-query";
import {
	eligibleArgoProfiles,
	resolveArgoConnectionPolicy,
	type ArgoConnectionChoice,
} from "./argo-connection-policy";
import { finiteRead } from "./finite-read-lifecycle";
import type {
	ArgoApplicationInspector,
	ArgoApplicationRef,
	ArgoConnectionStatus,
} from "./gitops-types";
import { queryKeys } from "./queryKeys";
import type { SavedArgoProfile, SettingsState } from "./settings";
import { getArgoApplicationInspector, getArgoConnectionStatus } from "./tauri-argo";
import type { TauriClient } from "./tauri-runtime";
import type { WorkspaceReadContext } from "./workspaceReadContext";

// ADR 0016 module 2: one entry point owns the profile status read, the workspace
// preference, the transport choice, and the whole-result inspection read.

export const ARGO_APPLICATION_API_VERSION = "argoproj.io/v1alpha1";

type ArgoConnectionStatusEntries = (readonly [string, ArgoConnectionStatus])[];

export interface ArgoConnectionStatusReadSpec {
	profiles: SavedArgoProfile[];
	queryKey: readonly unknown[];
	enabled: boolean;
}

/** Status of each profile; a profile whose status read fails is left out. */
export function argoConnectionStatusQueryOptions(
	client: TauriClient,
	spec: ArgoConnectionStatusReadSpec,
) {
	return {
		queryKey: spec.queryKey,
		queryFn: async (): Promise<ArgoConnectionStatusEntries> => {
			const results = await Promise.allSettled(
				spec.profiles.map(
					async (profile) =>
						[profile.id, await getArgoConnectionStatus(client, profile.id)] as const,
				),
			);
			return results.flatMap((result) =>
				result.status === "fulfilled" ? [result.value] : [],
			);
		},
		enabled: spec.enabled,
		staleTime: 5_000,
	};
}

/** The Application a view inspects; its workspace scope comes from the read context. */
export type ArgoApplicationTarget = Pick<
	ArgoApplicationRef,
	"name" | "namespace" | "project" | "resourceVersion" | "uid" | "apiVersion"
>;

export function argoApplicationRef(
	readContext: WorkspaceReadContext,
	target: ArgoApplicationTarget,
): ArgoApplicationRef {
	return {
		name: target.name,
		namespace: target.namespace ?? null,
		project: target.project ?? null,
		resourceVersion: target.resourceVersion ?? null,
		uid: target.uid ?? null,
		apiVersion: target.apiVersion ?? ARGO_APPLICATION_API_VERSION,
		context: readContext.clusterContext,
		workspaceId: readContext.workspaceId,
	};
}

export interface ArgoInspectionTarget {
	readContext: WorkspaceReadContext;
	/** Null while the view has no Application. */
	application: ArgoApplicationTarget | null;
	/** False while the view is hidden; nothing is read then. */
	enabled?: boolean;
}

export interface ArgoInspectionInput extends ArgoInspectionTarget {
	settings: Pick<SettingsState, "argoProfiles" | "argoConnectionPreferences" | "redactSecrets">;
}

export type ArgoApplicationInspectionRequest = Parameters<typeof getArgoApplicationInspector>[1];

export interface ArgoApplicationInspection {
	/** The normalized workspace preference, eligible profiles, and the transport they select. */
	choice: ArgoConnectionChoice<SavedArgoProfile>;
	selectedStatus: ArgoConnectionStatus | null;
	/** Automatic selection is waiting for profile statuses. */
	checkingProfiles: boolean;
	/** The view is reading, the choice is settled, and its transport can serve reads. */
	ready: boolean;
	/** What the choice resolves to, for reads that must use the same transport. */
	request: ArgoApplicationInspectionRequest | null;
	/** One whole result; a Kubernetes fallback keeps its Connected failure and provenance. */
	data: ArgoApplicationInspector | undefined;
	loading: boolean;
	refreshing: boolean;
	error: unknown;
}

export interface ArgoApplicationInspectionController {
	readonly current: ArgoApplicationInspection;
	update(input: ArgoInspectionInput): void;
	/** Reads run while at least one listener is subscribed. */
	subscribe(listener: (state: ArgoApplicationInspection) => void): () => void;
	refresh(): Promise<void>;
}

function inspectionQueryOptions(
	client: TauriClient,
	request: ArgoApplicationInspectionRequest,
	workspaceId: string,
	enabled: boolean,
) {
	const { application } = request;
	return {
		...finiteRead({
			queryKey: queryKeys.argoWorkspaceInspector(
				request.clusterContext,
				workspaceId,
				application.name,
				application.namespace,
				application.uid,
				request.redactSecrets === true,
				request.transport,
				request.connectionId,
				request.kubeconfigEnvVar,
			),
			label: "argo.inspection",
			read: (cancellable) => getArgoApplicationInspector(client, request, cancellable),
		}),
		enabled,
		staleTime: 30_000,
		refetchInterval: 15_000,
		refetchIntervalInBackground: false,
		retry: false,
		// Unredacted inspections leave the cache as soon as no view shows them.
		gcTime: request.redactSecrets ? undefined : 0,
	};
}

function inspectionRequest(
	readContext: WorkspaceReadContext,
	application: ArgoApplicationTarget,
	choice: ArgoConnectionChoice<SavedArgoProfile>,
	redactSecrets: boolean,
): ArgoApplicationInspectionRequest {
	const request: ArgoApplicationInspectionRequest = {
		clusterContext: readContext.clusterContext,
		kubeconfigEnvVar: readContext.kubeconfigSourceKey,
		transport: choice.transport,
		application: argoApplicationRef(readContext, application),
		redactSecrets,
	};
	if (choice.transport === "connected") request.connectionId = choice.connectionId ?? "";
	return request;
}

const idleRead = (label: string) => ({ queryKey: queryKeys.idle(label), enabled: false });

export function createArgoApplicationInspection(
	client: TauriClient,
	queryClient: QueryClient,
): ArgoApplicationInspectionController {
	const statuses = new QueryObserver<ArgoConnectionStatusEntries>(queryClient, idleRead("argo.statuses"));
	const inspector = new QueryObserver<ArgoApplicationInspector>(queryClient, idleRead("argo.inspection"));
	const listeners = new Set<(state: ArgoApplicationInspection) => void>();
	let input: ArgoInspectionInput | null = null;
	let current: ArgoApplicationInspection = {
		choice: resolveArgoConnectionPolicy({ profiles: [], clusterContext: "", workspaceId: "", kubeconfigSourceKey: "" }),
		selectedStatus: null,
		checkingProfiles: false,
		ready: false,
		request: null,
		data: undefined,
		loading: false,
		refreshing: false,
		error: null,
	};
	let stopObserving: (() => void) | null = null;
	let syncing = false;
	let resync = false;

	function step({ readContext, application, settings, enabled = true }: ArgoInspectionInput) {
		const { clusterContext, workspaceId, kubeconfigSourceKey } = readContext;
		const reading = enabled && readContext.sourceReady && application !== null;
		const profiles = eligibleArgoProfiles(settings.argoProfiles, clusterContext, workspaceId, kubeconfigSourceKey ?? "");
		const statusesEnabled = reading && profiles.length > 0;
		statuses.setOptions(argoConnectionStatusQueryOptions(client, {
			profiles,
			queryKey: queryKeys.argoConnectionStatuses(clusterContext, workspaceId, profiles.map((profile) => profile.id), kubeconfigSourceKey),
			enabled: statusesEnabled,
		}));
		const statusResult = statuses.getCurrentResult();
		const choice = resolveArgoConnectionPolicy({
			profiles,
			statuses: statusResult.data,
			clusterContext,
			workspaceId,
			kubeconfigSourceKey: kubeconfigSourceKey ?? "",
			preference: settings.argoConnectionPreferences[workspaceId],
		});
		const checkingProfiles = choice.preference.kind !== "kubernetes" && profiles.length > 0 && statusResult.isPending;
		const ready = reading && !checkingProfiles && (choice.transport === "kubernetes" || !choice.unavailable);
		const request = application && inspectionRequest(readContext, application, choice, settings.redactSecrets);
		inspector.setOptions(
			request ? inspectionQueryOptions(client, request, workspaceId, ready) : idleRead("argo.inspection"),
		);
		const result = inspector.getCurrentResult();
		const loading = (statusesEnabled && statusResult.isPending) || (ready && result.isPending);
		current = {
			choice,
			selectedStatus: statusResult.data?.find(([id]) => id === choice.connectionId)?.[1] ?? null,
			checkingProfiles,
			ready,
			request,
			data: request ? result.data : undefined,
			loading,
			refreshing: !loading && result.isFetching,
			error: result.isError ? result.error : null,
		};
	}

	// Setting observer options can report a new result synchronously; settle before notifying.
	function sync() {
		if (syncing) {
			resync = true;
			return;
		}
		syncing = true;
		try {
			do {
				resync = false;
				if (input) step(input);
			} while (resync);
		} finally {
			syncing = false;
		}
		for (const listener of listeners) listener(current);
	}

	return {
		get current() {
			return current;
		},
		update(next) {
			input = next;
			sync();
		},
		subscribe(listener) {
			listeners.add(listener);
			if (!stopObserving) {
				const stopStatuses = statuses.subscribe(sync);
				const stopInspector = inspector.subscribe(sync);
				stopObserving = () => {
					stopStatuses();
					stopInspector();
				};
			}
			sync();
			return () => {
				listeners.delete(listener);
				if (listeners.size > 0) return;
				stopObserving?.();
				stopObserving = null;
			};
		},
		async refresh() {
			if (current.ready) await inspector.refetch();
		},
	};
}
