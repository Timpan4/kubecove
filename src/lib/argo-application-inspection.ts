import type { ArgoConnectionChoice, ArgoConnectionPreference } from "./argo-connection-policy";
import {
	eligibleArgoProfiles,
	normalizeArgoConnectionPreference,
	resolveArgoConnectionPolicy,
} from "./argo-connection-policy";
import { createCancelScope, createFiniteReadRequest } from "./finite-read-lifecycle";
import type {
	ArgoApplicationInspector,
	ArgoApplicationRef,
	ArgoConnectionStatus,
} from "./gitops-types";
import { queryKeys } from "./queryKeys";
import type { SavedArgoProfile } from "./settings";
import { getArgoApplicationInspector, getArgoConnectionStatus } from "./tauri-argo";
import type { TauriClient } from "./tauri-runtime";

export interface ArgoConnectionStatusReadSpec {
	profiles: SavedArgoProfile[];
	queryKey: readonly unknown[];
	enabled: boolean;
}

export function buildArgoConnectionStatusReadSpec({
	profiles,
	clusterContext,
	workspaceId,
	kubeconfigEnvVar,
}: {
	profiles: readonly SavedArgoProfile[];
	clusterContext: string;
	workspaceId: string;
	kubeconfigEnvVar?: string;
}): ArgoConnectionStatusReadSpec {
	const eligibleProfiles = eligibleArgoProfiles(
		profiles,
		clusterContext,
		workspaceId,
		kubeconfigEnvVar ?? "",
	);
	return {
		profiles: eligibleProfiles,
		queryKey: queryKeys.argoConnectionStatuses(
			clusterContext,
			workspaceId,
			eligibleProfiles.map((profile) => profile.id),
			kubeconfigEnvVar,
		),
		enabled: eligibleProfiles.length > 0,
	};
}

export function argoConnectionStatusQueryOptions(
	client: TauriClient,
	spec: ArgoConnectionStatusReadSpec,
) {
	return {
		queryKey: spec.queryKey,
		queryFn: async () => {
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

export interface ArgoApplicationInspectionReadSpec {
	policy: ArgoConnectionChoice<SavedArgoProfile>;
	request: {
		clusterContext: string;
		kubeconfigEnvVar?: string;
		connectionId?: string;
		transport: "connected" | "kubernetes";
		application: ArgoApplicationRef;
		redactSecrets: boolean;
	};
	queryKey: readonly unknown[];
	cancelScope: string;
	ready: boolean;
	enabled: boolean;
	redactSecrets: boolean;
}

export function buildArgoApplicationInspectionReadSpec({
	profiles,
	statuses,
	statusesPending,
	preference,
	application,
	clusterContext,
	workspaceId,
	kubeconfigEnvVar,
	redactSecrets,
	enabled,
}: {
	profiles: readonly SavedArgoProfile[];
	statuses?: readonly (readonly [string, ArgoConnectionStatus])[];
	statusesPending: boolean;
	preference: ArgoConnectionPreference;
	application: ArgoApplicationRef;
	clusterContext: string;
	workspaceId: string;
	kubeconfigEnvVar?: string;
	redactSecrets: boolean;
	enabled: boolean;
}): ArgoApplicationInspectionReadSpec {
	const normalizedPreference = normalizeArgoConnectionPreference(preference);
	const policy = resolveArgoConnectionPolicy({
		profiles,
		statuses,
		clusterContext,
		workspaceId,
		kubeconfigSourceKey: kubeconfigEnvVar ?? "",
		preference: normalizedPreference,
	});
	const connectionId = policy.connectionId ?? "";
	const queryKey = queryKeys.argoWorkspaceInspector(
		clusterContext,
		workspaceId,
		application.name,
		application.namespace,
		application.uid,
		redactSecrets,
		policy.transport,
		connectionId,
		kubeconfigEnvVar,
	);
	const policyReady =
		normalizedPreference.kind === "kubernetes" ||
		policy.eligibleProfiles.length === 0 ||
		!statusesPending;
	const connectionReady =
		policyReady && (policy.transport === "kubernetes" || !policy.unavailable);
	const request: ArgoApplicationInspectionReadSpec["request"] = {
		clusterContext,
		kubeconfigEnvVar,
		transport: policy.transport,
		application,
		redactSecrets,
	};
	if (policy.transport === "connected") request.connectionId = connectionId;

	return {
		policy,
		request,
		queryKey,
		cancelScope: createCancelScope("argo-application-inspection", queryKey),
		ready: connectionReady,
		enabled: enabled && connectionReady,
		redactSecrets,
	};
}

export function argoApplicationInspectionQueryOptions(
	client: TauriClient,
	spec: ArgoApplicationInspectionReadSpec,
) {
	return {
		queryKey: spec.queryKey,
		queryFn: () =>
			getArgoApplicationInspector(
				client,
				spec.request,
				createFiniteReadRequest(spec.cancelScope, "argo-inspection"),
			),
		enabled: spec.enabled,
		staleTime: 30_000,
		refetchInterval: (query: { state: { data?: ArgoApplicationInspector } }) =>
			spec.request.transport === "connected" &&
			query.state.data?.transport === "connected"
				? 15_000
				: false,
		refetchIntervalInBackground: false,
		retry: false,
		gcTime: spec.redactSecrets ? undefined : 0,
	};
}
