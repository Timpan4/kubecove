import { useEffect, useMemo, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useQuery } from "@tanstack/react-query";
import {
	closeStreamChannel,
	createStreamChannel,
	getDynamicResourceDetails,
	getResourceDetails,
	getResourceYaml,
	listResourceEvents,
	startResourceEventWatch,
	startResourceWatch,
	stopStream,
	type TauriClient,
} from "../../lib/tauri";
import type {
	DiscoveredResourceKind,
	ResourceSummary,
	StreamMessage,
	WatchResourceKey,
} from "../../lib/types";
import { diagnosticLog, diagnosticResultSummary } from "../../lib/diagnostics";
import type { Tab } from "./constants";
import { shouldFetchResourceDetails, shouldFetchResourceEvents } from "./helpers";

interface UseResourceDetailsArgs {
	resource: ResourceSummary;
	activeTab: Tab;
	resourceKey: string;
	client: TauriClient;
	dynamicResourceKind: DiscoveredResourceKind | null;
}

export function useResourceDetails({
	resource,
	activeTab,
	resourceKey,
	client,
	dynamicResourceKind,
}: UseResourceDetailsArgs) {
	const queryClient = useQueryClient();
	const resourceDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const eventsDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const dynamicResourceKindKey = dynamicResourceKind
		? `${dynamicResourceKind.group}/${dynamicResourceKind.version}/${dynamicResourceKind.kind}/${dynamicResourceKind.plural}/${dynamicResourceKind.namespaced}`
		: null;
	const detailsEnabled = shouldFetchResourceDetails(resource);
	const yamlEnabled =
		activeTab === "yaml" &&
		!!resource.cluster &&
		!!resource.kind &&
		!!resource.name;
	const eventsEnabled = shouldFetchResourceEvents(resource);
	const detailsQueryKey = useMemo(
		() =>
			[
				"resource-details",
				dynamicResourceKindKey,
				resource.cluster,
				resource.apiVersion,
				resource.kind,
				resource.name,
				resource.namespace,
			] as const,
		[
			dynamicResourceKindKey,
			resource.apiVersion,
			resource.cluster,
			resource.kind,
			resource.name,
			resource.namespace,
		],
	);
	const yamlQueryKey = useMemo(
		() =>
			[
				"resource-yaml",
				dynamicResourceKindKey,
				resource.cluster,
				resource.apiVersion,
				resource.kind,
				resource.name,
				resource.namespace,
			] as const,
		[
			dynamicResourceKindKey,
			resource.apiVersion,
			resource.cluster,
			resource.kind,
			resource.name,
			resource.namespace,
		],
	);
	const eventsQueryKey = useMemo(
		() =>
			[
				"resource-events",
				resource.cluster,
				resource.apiVersion,
				resource.kind,
				resource.name,
				resource.namespace,
			] as const,
		[
			resource.apiVersion,
			resource.cluster,
			resource.kind,
			resource.name,
			resource.namespace,
		],
	);

	const detailsQuery = useQuery({
		queryKey: detailsQueryKey,
		queryFn: async () => {
			const started = performance.now();
			diagnosticLog("detail.details.fetch.start", { key: resourceKey });
			const result = dynamicResourceKind
				? await getDynamicResourceDetails(
						client,
						resource.cluster,
						dynamicResourceKind,
						resource.name,
						resource.namespace ?? undefined,
					)
				: await getResourceDetails(
						client,
						resource.cluster,
						resource.kind,
						resource.name,
						resource.namespace ?? undefined,
					);
			diagnosticLog("detail.details.fetch.done", {
				key: resourceKey,
				ms: Math.round(performance.now() - started),
				result: diagnosticResultSummary(result),
			});
			return result;
		},
		enabled: detailsEnabled,
		retry: false,
	});

	const yamlQuery = useQuery({
		queryKey: yamlQueryKey,
		queryFn: async () => {
			const started = performance.now();
			diagnosticLog("detail.yaml.fetch.start", { key: resourceKey });
			const result = dynamicResourceKind
				? detailsQuery.data?.yaml ??
					(
						await getDynamicResourceDetails(
							client,
							resource.cluster,
							dynamicResourceKind,
							resource.name,
							resource.namespace ?? undefined,
						)
					).yaml
				: await getResourceYaml(
						client,
						resource.cluster,
						resource.kind,
						resource.name,
						resource.namespace ?? undefined,
					);
			diagnosticLog("detail.yaml.fetch.done", {
				key: resourceKey,
				ms: Math.round(performance.now() - started),
				result: diagnosticResultSummary(result),
			});
			return result;
		},
		enabled: yamlEnabled,
		retry: false,
	});

	const eventsQuery = useQuery({
		queryKey: eventsQueryKey,
		queryFn: async () => {
			const started = performance.now();
			diagnosticLog("detail.events.fetch.start", { key: resourceKey });
			const result = await listResourceEvents(
				client,
				resource.cluster,
				resource.kind,
				resource.name,
				resource.namespace ?? undefined,
			);
			diagnosticLog("detail.events.fetch.done", {
				key: resourceKey,
				ms: Math.round(performance.now() - started),
				result: diagnosticResultSummary(result),
			});
			return result;
		},
		enabled: eventsEnabled,
		retry: false,
	});

	useEffect(() => {
		if (!detailsEnabled) return;
		let cancelled = false;
		let streamId: string | null = null;
		const watchKey: WatchResourceKey = dynamicResourceKind
			? {
					resourceKind: {
						kind: dynamicResourceKind.kind,
						group: dynamicResourceKind.group,
						version: dynamicResourceKind.version,
						apiVersion: dynamicResourceKind.apiVersion,
						plural: dynamicResourceKind.plural,
						namespaced: dynamicResourceKind.namespaced,
					},
					namespace: resource.namespace ?? undefined,
				}
			: {
					resourceKind: { kind: resource.kind },
					namespace: resource.namespace ?? undefined,
				};

		const invalidateSoon = () => {
			if (resourceDebounceRef.current) clearTimeout(resourceDebounceRef.current);
			resourceDebounceRef.current = setTimeout(() => {
				void queryClient.invalidateQueries({ queryKey: detailsQueryKey });
				if (activeTab === "yaml") {
					void queryClient.invalidateQueries({ queryKey: yamlQueryKey });
				}
			}, 250);
		};

		const channel = createStreamChannel((event: StreamMessage) => {
			if (cancelled || event.type !== "resourceChanged") return;
			if (event.target.name && event.target.name !== resource.name) return;
			if (
				resource.namespace &&
				event.target.namespace &&
				event.target.namespace !== resource.namespace
			) {
				return;
			}
			invalidateSoon();
		});

		void startResourceWatch(client, resource.cluster, [watchKey], channel).then((id) => {
			if (cancelled) {
				void stopStream(client, id);
				return;
			}
			streamId = id;
		}).catch((err: unknown) => {
			diagnosticLog("detail.resource.watch.error", {
				key: resourceKey,
				error: err instanceof Error ? err.message : String(err),
			});
		});

		return () => {
			cancelled = true;
			if (resourceDebounceRef.current) clearTimeout(resourceDebounceRef.current);
			if (streamId) void stopStream(client, streamId);
			closeStreamChannel(channel);
		};
	}, [
		activeTab,
		client,
		detailsEnabled,
		dynamicResourceKind,
		detailsQueryKey,
		queryClient,
		resource,
		resourceKey,
		yamlQueryKey,
	]);

	useEffect(() => {
		if (!eventsEnabled) return;
		let cancelled = false;
		let streamId: string | null = null;

		const invalidateSoon = () => {
			if (eventsDebounceRef.current) clearTimeout(eventsDebounceRef.current);
			eventsDebounceRef.current = setTimeout(() => {
				void queryClient.invalidateQueries({ queryKey: eventsQueryKey });
			}, 250);
		};

		const channel = createStreamChannel((event: StreamMessage) => {
			if (cancelled) return;
			if (event.type === "resourceEventsChanged") {
				invalidateSoon();
			}
		});

		void startResourceEventWatch(
			client,
			resource.cluster,
			resource.kind,
			resource.name,
			resource.namespace ?? undefined,
			channel,
		).then((id) => {
			if (cancelled) {
				void stopStream(client, id);
				return;
			}
			streamId = id;
		}).catch((err: unknown) => {
			diagnosticLog("detail.events.watch.error", {
				key: resourceKey,
				error: err instanceof Error ? err.message : String(err),
			});
		});

		return () => {
			cancelled = true;
			if (eventsDebounceRef.current) clearTimeout(eventsDebounceRef.current);
			if (streamId) void stopStream(client, streamId);
			closeStreamChannel(channel);
		};
	}, [
		client,
		eventsEnabled,
		eventsQueryKey,
		queryClient,
		resource.cluster,
		resource.kind,
		resource.name,
		resource.namespace,
		resourceKey,
	]);

	return {
		detailsEnabled,
		yamlEnabled,
		eventsEnabled,
		detailsQuery,
		yamlQuery,
		eventsQuery,
	};
}
