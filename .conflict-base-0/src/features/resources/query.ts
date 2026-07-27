import { diagnosticLog } from "@/lib/diagnostics";
import { createCancellableRequest } from "@/lib/cancellable-loads";
import { withForegroundLoad } from "@/lib/foreground-loading";
import { createTauriClient, isAppError, listResourceScope } from "@/lib/tauri";
import type {
	CancellableRequest,
	ResourceListRequest,
	ResourceSummary,
} from "@/lib/types";
import {
	isDiscoveredResourceKind,
	resourceKindLabel,
	type FetchKey,
} from "./helpers";

function fetchKeyRequest({ kind, namespace }: FetchKey): ResourceListRequest {
	const normalizedNamespace = namespace === "" ? undefined : namespace;
	if (isDiscoveredResourceKind(kind)) {
		return { resourceKind: kind, namespace: normalizedNamespace };
	}
	return { kind, namespace: normalizedNamespace };
}

export async function fetchResourcePage(
	clusterContext: string,
	fetchKeys: FetchKey[],
	kubeconfigEnvVar?: string,
	cancelScope?: string,
): Promise<ResourceSummary[]> {
	const started = performance.now();
	const cancellable: CancellableRequest | undefined = cancelScope
		? createCancellableRequest(cancelScope, "resources")
		: undefined;
	diagnosticLog("resources.fetch.start", {
		cluster: clusterContext,
		fetches: fetchKeys.length,
		kinds: fetchKeys.map((key) => resourceKindLabel(key.kind)).join("|"),
	});
	const client = createTauriClient();
	const rows = await withForegroundLoad("resources", () =>
		listResourceScope(
			client,
			clusterContext,
			fetchKeys.map(fetchKeyRequest),
			kubeconfigEnvVar,
			cancellable,
		).catch((error) => {
			if (isAppError(error) && error.kind === "cancelled") {
				diagnosticLog("resources.scope.cancel", {
					fetches: fetchKeys.length,
				});
			}
			throw error;
		}),
	);
	diagnosticLog("resources.fetch.done", {
		cluster: clusterContext,
		fetches: fetchKeys.length,
		rows: rows.length,
		ms: Math.round(performance.now() - started),
	});
	return rows;
}
