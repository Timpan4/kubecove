import { diagnosticLog } from "@/lib/diagnostics";
import { createTauriClient, listResourceScope } from "@/lib/tauri";
import type { ResourceListRequest, ResourceSummary } from "@/lib/types";
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
): Promise<ResourceSummary[]> {
	const started = performance.now();
	diagnosticLog("resources.fetch.start", {
		cluster: clusterContext,
		fetches: fetchKeys.length,
		kinds: fetchKeys.map((key) => resourceKindLabel(key.kind)).join("|"),
	});
	const client = createTauriClient();
	const rows = await listResourceScope(
		client,
		clusterContext,
		fetchKeys.map(fetchKeyRequest),
		kubeconfigEnvVar,
	);
	diagnosticLog("resources.fetch.done", {
		cluster: clusterContext,
		fetches: fetchKeys.length,
		rows: rows.length,
		ms: Math.round(performance.now() - started),
	});
	return rows;
}
