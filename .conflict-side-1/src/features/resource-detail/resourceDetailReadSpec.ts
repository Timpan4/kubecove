import { requiredPermissionForResource } from "@/features/rbac";
import { createCancelScope } from "@/lib/cancellable-loads";
import { queryKeys } from "@/lib/queryKeys";
import type {
	DiscoveredResourceKind,
	RbacAccessReviewTarget,
	ResourceSummary,
	WatchResourceKey,
	YamlEncoding,
	YamlViewMode,
} from "@/lib/types";
import {
	dynamicResourceKindFromSummary,
	shouldFetchResourceDetails,
	shouldFetchResourceEvents,
} from "./helpers";

export interface ResourceDetailReadSpec {
	identity: { key: string };
	dynamicKind: DiscoveredResourceKind | null;
	detailReadPermission: RbacAccessReviewTarget | null;
	detailsEnabled: boolean;
	eventsEnabled: boolean;
	detailsQueryKey: readonly unknown[];
	eventsQueryKey: readonly unknown[];
	detailsCancelScope: string;
	eventsCancelScope: string;
	resourceWatchKey: WatchResourceKey;
	eventWatch: {
		cluster: string;
		kind: string;
		name: string;
		namespace?: string;
	};
}

function resourceIdentity(resource: ResourceSummary): string {
	return `${resource.cluster}:${resource.apiVersion ?? ""}:${resource.kind}:${resource.namespace ?? ""}:${resource.name}`;
}

function dynamicKindKey(dynamicKind: DiscoveredResourceKind | null): string {
	return dynamicKind
		? `${dynamicKind.group}/${dynamicKind.version}/${dynamicKind.kind}/${dynamicKind.plural}/${dynamicKind.namespaced}`
		: "";
}

function resourceWatchKey(
	resource: ResourceSummary,
	dynamicKind: DiscoveredResourceKind | null,
): WatchResourceKey {
	return dynamicKind
		? {
				resourceKind: {
					kind: dynamicKind.kind,
					group: dynamicKind.group,
					version: dynamicKind.version,
					apiVersion: dynamicKind.apiVersion,
					plural: dynamicKind.plural,
					namespaced: dynamicKind.namespaced,
				},
				namespace: resource.namespace ?? undefined,
			}
		: {
				resourceKind: { kind: resource.kind },
				namespace: resource.namespace ?? undefined,
			};
}

export function buildResourceDetailReadSpec(
	resource: ResourceSummary,
	kubeconfigSourceKey: string | undefined,
	yamlViewMode: YamlViewMode,
	yamlEncoding: YamlEncoding,
): ResourceDetailReadSpec {
	const dynamicKind = dynamicResourceKindFromSummary(resource);
	const selectedDynamicKindKey = dynamicKindKey(dynamicKind);
	const detailsQueryKey = queryKeys.resourceDetails(
		resource,
		selectedDynamicKindKey,
		kubeconfigSourceKey,
		yamlViewMode,
		yamlEncoding,
	);
	const eventsQueryKey = queryKeys.resourceEvents(resource, kubeconfigSourceKey);

	return {
		identity: { key: resourceIdentity(resource) },
		dynamicKind,
		detailReadPermission: requiredPermissionForResource(resource, "get"),
		detailsEnabled: shouldFetchResourceDetails(resource),
		eventsEnabled: shouldFetchResourceEvents(resource),
		detailsQueryKey,
		eventsQueryKey,
		detailsCancelScope: createCancelScope("resource-details", detailsQueryKey),
		eventsCancelScope: createCancelScope("resource-events", eventsQueryKey),
		resourceWatchKey: resourceWatchKey(resource, dynamicKind),
		eventWatch: {
			cluster: resource.cluster,
			kind: resource.kind,
			name: resource.name,
			namespace: resource.namespace ?? undefined,
		},
	};
}
