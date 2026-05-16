import { useQuery } from "@tanstack/react-query";
import {
	getDynamicResourceDetails,
	getResourceDetails,
	getResourceYaml,
	listResourceEvents,
	type TauriClient,
} from "../../lib/tauri";
import type { DiscoveredResourceKind, ResourceSummary } from "../../lib/types";
import { diagnosticLog, diagnosticResultSummary } from "../../lib/diagnostics";
import type { Tab } from "./constants";
import { shouldFetchResourceDetails } from "./helpers";

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
	const dynamicResourceKindKey = dynamicResourceKind
		? `${dynamicResourceKind.group}/${dynamicResourceKind.version}/${dynamicResourceKind.kind}/${dynamicResourceKind.plural}/${dynamicResourceKind.namespaced}`
		: null;
	const detailsEnabled = shouldFetchResourceDetails(resource);
	const yamlEnabled =
		activeTab === "yaml" &&
		!!resource.cluster &&
		!!resource.kind &&
		!!resource.name;
	const eventsEnabled =
		activeTab === "events" &&
		!!resource.cluster &&
		!!resource.kind &&
		!!resource.name;

	const detailsQuery = useQuery({
		queryKey: [
			"resource-details",
			dynamicResourceKindKey,
			resource.cluster,
			resource.apiVersion,
			resource.kind,
			resource.name,
			resource.namespace,
		],
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
		queryKey: [
			"resource-yaml",
			dynamicResourceKindKey,
			resource.cluster,
			resource.apiVersion,
			resource.kind,
			resource.name,
			resource.namespace,
		],
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
		queryKey: [
			"resource-events",
			resource.cluster,
			resource.apiVersion,
			resource.kind,
			resource.name,
			resource.namespace,
		],
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

	return {
		detailsEnabled,
		yamlEnabled,
		eventsEnabled,
		detailsQuery,
		yamlQuery,
		eventsQuery,
	};
}
