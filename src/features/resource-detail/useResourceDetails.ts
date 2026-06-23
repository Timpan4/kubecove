import { useEffect, useMemo, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useQuery } from "@tanstack/react-query";
import {
	createCancellableRequest,
	createCancelScope,
	useCancelBackendScopes,
} from "@/lib/cancellable-loads";
import { withForegroundLoad } from "@/lib/foreground-loading";
import {
	closeStreamChannel,
	createStreamChannel,
	getDynamicResourceDetails,
	getResourceDetails,
	getResourceYaml,
	isAppError,
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
	YamlEncoding,
	YamlViewMode,
} from "../../lib/types";
import { diagnosticLog, diagnosticResultSummary } from "../../lib/diagnostics";
import { useSettingsState } from "../../lib/settings";
import { queryKeys } from "../../lib/queryKeys";
import type { Tab } from "./constants";
import { shouldFetchResourceDetails, shouldFetchResourceEvents } from "./helpers";

interface UseResourceDetailsArgs {
	resource: ResourceSummary;
	activeTab: Tab;
	resourceKey: string;
	client: TauriClient;
	dynamicResourceKind: DiscoveredResourceKind | null;
	yamlViewMode: YamlViewMode;
	yamlEncoding: YamlEncoding;
}

export function useResourceDetails({
	resource,
	activeTab,
	resourceKey,
	client,
	dynamicResourceKind,
	yamlViewMode,
	yamlEncoding,
}: UseResourceDetailsArgs) {
	const queryClient = useQueryClient();
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
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
			queryKeys.resourceDetails(
				resource,
				dynamicResourceKindKey,
				kubeconfigEnvVar,
				yamlViewMode,
				yamlEncoding,
			),
		[
			dynamicResourceKindKey,
			kubeconfigEnvVar,
			resource.apiVersion,
			resource.cluster,
			resource.kind,
			resource.name,
			resource.namespace,
			yamlViewMode,
			yamlEncoding,
		],
	);
	const yamlQueryKey = useMemo(
		() =>
			queryKeys.resourceYaml(
				resource,
				dynamicResourceKindKey,
				kubeconfigEnvVar,
				yamlViewMode,
				yamlEncoding,
			),
		[
			dynamicResourceKindKey,
			kubeconfigEnvVar,
			resource.apiVersion,
			resource.cluster,
			resource.kind,
			resource.name,
			resource.namespace,
			yamlViewMode,
			yamlEncoding,
		],
	);
	const eventsQueryKey = useMemo(
		() => queryKeys.resourceEvents(resource, kubeconfigEnvVar),
		[
			kubeconfigEnvVar,
			resource.apiVersion,
			resource.cluster,
			resource.kind,
			resource.name,
			resource.namespace,
		],
	);
	const detailsCancelScope = useMemo(
		() => createCancelScope("resource-details", detailsQueryKey),
		[detailsQueryKey],
	);
	const yamlCancelScope = useMemo(
		() => createCancelScope("resource-yaml", yamlQueryKey),
		[yamlQueryKey],
	);
	const eventsCancelScope = useMemo(
		() => createCancelScope("resource-events", eventsQueryKey),
		[eventsQueryKey],
	);
	const cancelEntries = useMemo(
		() => [
			{
				cancelScope: detailsCancelScope,
				queryKey: detailsQueryKey,
				event: "detail.details.cancel",
			},
			{
				cancelScope: yamlCancelScope,
				queryKey: yamlQueryKey,
				event: "detail.yaml.cancel",
			},
			{
				cancelScope: eventsCancelScope,
				queryKey: eventsQueryKey,
				event: "detail.events.cancel",
			},
		],
		[
			detailsCancelScope,
			detailsQueryKey,
			eventsCancelScope,
			eventsQueryKey,
			yamlCancelScope,
			yamlQueryKey,
		],
	);
	useCancelBackendScopes(client, cancelEntries);

	const {
		data: detailsData,
		isLoading: detailsLoading,
		isError: detailsIsError,
		error: detailsError,
	} = useQuery({
		queryKey: detailsQueryKey,
		queryFn: async () => {
			const started = performance.now();
			diagnosticLog("detail.details.fetch.start", { key: resourceKey });
			const result = await withForegroundLoad("resource-details", () =>
				(dynamicResourceKind
					? getDynamicResourceDetails(
							client,
							resource.cluster,
							dynamicResourceKind,
							resource.name,
							resource.namespace ?? undefined,
							kubeconfigEnvVar,
							yamlViewMode,
							yamlEncoding,
							createCancellableRequest(detailsCancelScope, "details"),
						)
					: getResourceDetails(
							client,
							resource.cluster,
							resource.kind,
							resource.name,
							resource.namespace ?? undefined,
							kubeconfigEnvVar,
							yamlViewMode,
							yamlEncoding,
							createCancellableRequest(detailsCancelScope, "details"),
						)
				).catch((error) => {
					if (isAppError(error) && error.kind === "cancelled") {
						diagnosticLog("detail.details.cancel", { key: resourceKey });
					}
					throw error;
				}),
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

	const {
		data: yamlData,
		isLoading: yamlLoading,
		isError: yamlIsError,
		error: yamlError,
	} = useQuery({
		queryKey: yamlQueryKey,
		queryFn: async () => {
			const started = performance.now();
			diagnosticLog("detail.yaml.fetch.start", { key: resourceKey });
			const result = await withForegroundLoad("resource-yaml", () =>
				(dynamicResourceKind
					? Promise.resolve(detailsData?.yaml).then(async (cachedYaml) => {
							if (cachedYaml) return cachedYaml;
							return (
								await getDynamicResourceDetails(
									client,
									resource.cluster,
									dynamicResourceKind,
									resource.name,
									resource.namespace ?? undefined,
									kubeconfigEnvVar,
									yamlViewMode,
									yamlEncoding,
									createCancellableRequest(yamlCancelScope, "yaml"),
								)
							).yaml;
						})
					: getResourceYaml(
							client,
							resource.cluster,
							resource.kind,
							resource.name,
							resource.namespace ?? undefined,
							kubeconfigEnvVar,
							yamlViewMode,
							yamlEncoding,
							createCancellableRequest(yamlCancelScope, "yaml"),
						)
				).catch((error) => {
					if (isAppError(error) && error.kind === "cancelled") {
						diagnosticLog("detail.yaml.cancel", { key: resourceKey });
					}
					throw error;
				}),
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

	const {
		data: eventsData,
		isLoading: eventsLoading,
		isError: eventsIsError,
		error: eventsError,
	} = useQuery({
		queryKey: eventsQueryKey,
		queryFn: async () => {
			const started = performance.now();
			diagnosticLog("detail.events.fetch.start", { key: resourceKey });
			const result = await withForegroundLoad("resource-events", () =>
				listResourceEvents(
					client,
					resource.cluster,
					resource.kind,
					resource.name,
					resource.namespace ?? undefined,
					kubeconfigEnvVar,
					createCancellableRequest(eventsCancelScope, "events"),
				).catch((error) => {
					if (isAppError(error) && error.kind === "cancelled") {
						diagnosticLog("detail.events.cancel", { key: resourceKey });
					}
					throw error;
				}),
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

		void startResourceWatch(
			client,
			resource.cluster,
			[watchKey],
			channel,
			kubeconfigEnvVar,
		).then((id) => {
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
		kubeconfigEnvVar,
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
			kubeconfigEnvVar,
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
		kubeconfigEnvVar,
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
		detailsQuery: {
			data: detailsData,
			isLoading: detailsLoading,
			isError: detailsIsError,
			error: detailsError,
		},
		yamlQuery: {
			data: yamlData,
			isLoading: yamlLoading,
			isError: yamlIsError,
			error: yamlError,
		},
		eventsQuery: {
			data: eventsData,
			isLoading: eventsLoading,
			isError: eventsIsError,
			error: eventsError,
		},
	};
}
