import { useEffect, useMemo } from "react";
import { useQueryClient, type QueryKey } from "@tanstack/react-query";
import { diagnosticLog } from "./diagnostics";
import { cancelBackendRequests, type TauriClient } from "./tauri";
import type { CancellableRequest } from "./types";

let requestSequence = 0;

export function createCancelScope(label: string, parts: unknown): string {
	return `${label}:${JSON.stringify(parts)}`;
}

export function createCancellableRequest(
	cancelScope: string,
	label: string,
): CancellableRequest {
	requestSequence = (requestSequence + 1) % Number.MAX_SAFE_INTEGER;
	return {
		cancelScope,
		requestId: `${label}-${Date.now().toString(36)}-${requestSequence.toString(36)}`,
	};
}

export function cancellableArg(
	request?: CancellableRequest,
): Partial<CancellableRequest> {
	if (!request) return {};
	return request;
}

interface BackendCancelEntry {
	cancelScope: string;
	queryKey?: QueryKey;
	event: string;
}

const pendingScopeCancels = new Map<string, ReturnType<typeof setTimeout>>();

function cancelPendingScopeCancel(cancelScope: string) {
	const timer = pendingScopeCancels.get(cancelScope);
	if (!timer) return;
	clearTimeout(timer);
	pendingScopeCancels.delete(cancelScope);
}

function cancelEntry(
	client: TauriClient,
	queryClient: ReturnType<typeof useQueryClient>,
	entry: BackendCancelEntry,
) {
	if (entry.queryKey) {
		void queryClient.cancelQueries({
			queryKey: entry.queryKey,
			exact: true,
		});
	}
	void cancelBackendRequests(client, entry.cancelScope)
		.then((result) => {
			if (result.cancelled > 0) {
				diagnosticLog(entry.event, {
					cancelled: result.cancelled,
				});
			}
		})
		.catch((error) => {
			diagnosticLog(`${entry.event}.error`, {
				error: error instanceof Error ? error.message : String(error),
			});
		});
}

export function useCancelBackendScopes(
	client: TauriClient,
	entries: BackendCancelEntry[],
) {
	const queryClient = useQueryClient();
	const signature = useMemo(
		() =>
			JSON.stringify(
				entries.map((entry) => ({
					cancelScope: entry.cancelScope,
					queryKey: entry.queryKey,
					event: entry.event,
				})),
			),
		[entries],
	);

	useEffect(() => {
		const currentEntries = entries;
		for (const entry of currentEntries) {
			cancelPendingScopeCancel(entry.cancelScope);
		}
		return () => {
			for (const entry of currentEntries) {
				cancelPendingScopeCancel(entry.cancelScope);
				const timer = setTimeout(() => {
					pendingScopeCancels.delete(entry.cancelScope);
					cancelEntry(client, queryClient, entry);
				}, 0);
				pendingScopeCancels.set(entry.cancelScope, timer);
			}
		};
	}, [client, entries, queryClient, signature]);
}
