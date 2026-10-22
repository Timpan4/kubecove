import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	topologyWatchKeys,
} from "@/features/resources/helpers";
import { kindFetchKeys, kindKey, kindWatchKeys, mergeWatchKeys } from "@/lib/kind-identity";
import type { ResourceScopeOption } from "@/features/resources/resourceBrowserModel";
import { filterResourceScopeOptions } from "@/features/resources/resourceBrowserModel";
import { buildResourceBrowserReadSpecs } from "@/features/resources/resourceBrowserReadSpecs";
import {
	buildWorkspaceFetchKeys,
	buildWorkspaceFetchPlans,
} from "@/features/workspaces/query";
import type {
	DiscoveredResourceKind,
	ResourceKindSelection,
} from "@/lib/types";
import type { WorkspaceScope } from "@/lib/workspace-model";

const namespaces = Array.from(
	{ length: 100 },
	(_, index) => `namespace-${index}`,
);
const discoveredKinds: DiscoveredResourceKind[] = Array.from(
	{ length: 100 },
	(_, index) => ({
		group: `operator-${index % 40}.example.com`,
		version: index % 2 === 0 ? "v1" : "v1beta1",
		apiVersion: `operator-${index % 40}.example.com/${index % 2 === 0 ? "v1" : "v1beta1"}`,
		kind: `Widget${index}`,
		plural: `widgets${index}`,
		namespaced: index % 5 !== 0,
	}),
);
const kinds: ResourceKindSelection[] = [
	"Pod",
	"Deployment",
	"ReplicaSet",
	"Service",
	"ConfigMap",
	"Secret",
	"Node",
	...discoveredKinds,
];
const fetchKeys = kindFetchKeys(namespaces, kinds);
const fetchWatchKeys = kindWatchKeys(fetchKeys);
const topologyOnlyWatchKeys = topologyWatchKeys(namespaces);
const mergedWatchKeys = mergeWatchKeys(fetchWatchKeys, topologyOnlyWatchKeys);
const options: ResourceScopeOption[] = [
	...namespaces.map((namespace) => ({ key: namespace, label: namespace })),
	...[
		"Pod",
		"Deployment",
		"ReplicaSet",
		"Service",
		"ConfigMap",
		"Secret",
		"Node",
		...discoveredKinds.map((kind) => `${kind.kind} ${kind.apiVersion}`),
	].map((label, index) => ({
		key: `kind-${index}`,
		label,
	})),
];
const workspaceScope: WorkspaceScope = {
	clusterContext: "prod/context-0",
	clusterGroup: {
		id: "production",
		name: "Production",
		members: ["prod/context-0", "prod/context-1", "prod/context-2"],
	},
	namespaces,
	kinds,
	argoAppFilter: "",
	layout: "overview",
};
const availableNamespaces = namespaces.filter((_, index) => index % 2 === 0);
const scopeFetchKeys = buildWorkspaceFetchKeys(
	workspaceScope,
	availableNamespaces,
);
const workspacePlans = buildWorkspaceFetchPlans(workspaceScope);
const readSpecs = buildResourceBrowserReadSpecs({
	clusterContext: "prod/context-0",
	kubeconfigSourceKey: "source-0",
	fetchKeys,
	namespaces,
	topologyMode: "ownership",
	mapPanelOpen: true,
	sourceReady: true,
	customResourcesEnabled: true,
});

function fetchKeyIdentity(key: (typeof fetchKeys)[number]): string {
	return `${kindKey(key.kind)}:${key.namespace ?? ""}`;
}

function watchKeyIdentity(key: (typeof fetchWatchKeys)[number]): string {
	const kind = key.resourceKind;
	return [
		kind.kind,
		kind.group ?? "",
		kind.version ?? "",
		kind.apiVersion ?? "",
		kind.plural ?? "",
		key.namespace ?? "",
	].join(":");
}

const expectedReadScope = fetchKeys
	.map(fetchKeyIdentity)
	.sort((left, right) => left.localeCompare(right));
const firstSeenWatchIdentities = new Set<string>();
const expectedMergedWatchScope: string[] = [];
for (const key of [...fetchWatchKeys, ...topologyOnlyWatchKeys]) {
	const identity = watchKeyIdentity(key);
	if (firstSeenWatchIdentities.has(identity)) continue;
	firstSeenWatchIdentities.add(identity);
	expectedMergedWatchScope.push(identity);
}

assert.equal(
	fetchKeys.length,
	6 * namespaces.length + 1 + 80 * namespaces.length + 20,
);
assert.deepEqual(readSpecs.resourceQueryKey.slice(3), expectedReadScope);
assert.deepEqual(
	mergedWatchKeys.map(watchKeyIdentity),
	expectedMergedWatchScope,
);
assert(mergedWatchKeys.length < fetchWatchKeys.length + topologyOnlyWatchKeys.length);
assert.equal(
	new Set(mergedWatchKeys.map(watchKeyIdentity)).size,
	mergedWatchKeys.length,
);
assert(
	fetchWatchKeys.some(
		(key) => key.resourceKind.apiVersion === discoveredKinds[1]?.apiVersion,
	),
);
assert.equal(scopeFetchKeys.length, 6 * 50 + 1 + 80 * 50 + 20);
assert.equal(workspacePlans.length, 3);
assert(
	workspacePlans.every(
		(plan) =>
			plan.requests.length ===
			6 * namespaces.length + 1 + 80 * namespaces.length + 20,
	),
);
assert(
	filterResourceScopeOptions(options, "widget99").some(
		(option) => option.key === "kind-106",
	),
);

describe("F04 resource scope and read planning with folded F33 workspace fetch plans", () => {
	bench("F04 build canonical fetch/watch keys and read specs", () => {
		const nextFetchKeys = kindFetchKeys(namespaces, kinds);
		const nextWatchKeys = kindWatchKeys(nextFetchKeys);
		mergeWatchKeys(nextWatchKeys, topologyWatchKeys(namespaces));
		buildResourceBrowserReadSpecs({
			clusterContext: "prod/context-0",
			kubeconfigSourceKey: "source-0",
			fetchKeys: nextFetchKeys,
			namespaces,
			topologyMode: "ownership",
			mapPanelOpen: true,
			sourceReady: true,
			customResourcesEnabled: true,
		});
	});

	bench("F04 retained selector options search", () => {
		filterResourceScopeOptions(options, "widget99");
	});

	bench("F33 workspace namespace, kind, and context fetch planning", () => {
		buildWorkspaceFetchKeys(workspaceScope, availableNamespaces);
		buildWorkspaceFetchPlans(workspaceScope);
	});
});
