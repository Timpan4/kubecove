import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	appendPresentCustomResourceKinds,
	buildSidebarTree,
	resourceBrowserAvailableKinds,
} from "@/app/svelte/workspaceShellModel";
import {
	buildExpandedNamespaceChildren,
	buildNamespaceTreeNode,
	buildShallowNamespaceTreeNode,
	customResourceErrorsByExpandedNamespace,
	customResourcesByExpandedNamespace,
	extraDiscoveredKinds,
	filterCustomResourceKinds,
	pendingCustomResourceNamespaces,
} from "@/components/sidebar-tree-helpers";
import { isDiscoveredResourceKind } from "@/features/resources/helpers";
import type {
	DiscoveredResourceKind,
	ResourceKindSelection,
} from "@/lib/types";

const namespaces = Array.from(
	{ length: 1_000 },
	(_, index) => `namespace-${index}`,
);
const extraKinds: DiscoveredResourceKind[] = Array.from(
	{ length: 100 },
	(_, index) => ({
		group: `operator-${index % 40}.example.com`,
		version: index % 2 === 0 ? "v1" : "v1beta1",
		apiVersion: `operator-${index % 40}.example.com/${index % 2 === 0 ? "v1" : "v1beta1"}`,
		kind: `Widget${index}`,
		plural: `widgets${index}`,
		shortNames: [`wdg${index}`],
		namespaced: true,
	}),
);
const firstExtraKind = extraKinds[0];
if (!firstExtraKind) throw new Error("Missing discovered-kind fixture");
const duplicateDiscoveryObservations = [
	...extraKinds,
	{ ...firstExtraKind, shortNames: ["duplicate-observation"] },
	firstExtraKind,
];
const presentKindsByNamespace = namespaces.map((namespace, index) => ({
	namespace,
	kinds: extraKinds.filter(
		(kind) => kind.group === `operator-${index % 40}.example.com`,
	),
}));
const namespaceQueryStates = presentKindsByNamespace.map(
	({ namespace, kinds }, index) => ({
		data: index % 97 === 0 ? undefined : kinds,
		error:
			index % 97 === 0 ? new Error(`discovery failed for ${namespace}`) : null,
		isError: index % 97 === 0,
		isPending: index % 113 === 0 && index % 97 !== 0,
	}),
);
const namespaceNodes = namespaces.map((namespace) =>
	buildShallowNamespaceTreeNode(namespace),
);
const allExpandedChildren = presentKindsByNamespace.map(
	({ namespace, kinds }) => buildNamespaceTreeNode(namespace, kinds),
);
const resourceSets = customResourcesByExpandedNamespace(
	namespaces,
	namespaceQueryStates,
);
const errorSets = customResourceErrorsByExpandedNamespace(
	namespaces,
	namespaceQueryStates,
);
const pendingNamespaces = pendingCustomResourceNamespaces(
	namespaces,
	namespaceQueryStates,
);
const lazyChildren = namespaces.map((namespace) =>
	buildExpandedNamespaceChildren(
		namespace,
		resourceSets.get(namespace) ?? [],
		errorSets.get(namespace),
		pendingNamespaces.has(namespace),
	),
);
const availableKinds = resourceBrowserAvailableKinds(
	["Pod", "Deployment"],
	duplicateDiscoveryObservations,
	false,
);
const fixedScopeKinds = resourceBrowserAvailableKinds(
	["Pod", "Deployment"],
	duplicateDiscoveryObservations,
	true,
);
const appendedKinds = appendPresentCustomResourceKinds(
	["Pod"],
	duplicateDiscoveryObservations,
);
function discoveredKindIdentity(kind: ResourceKindSelection): string {
	return isDiscoveredResourceKind(kind)
		? `dynamic:${kind.apiVersion}:${kind.plural}:${kind.kind}`
		: `typed:${kind}`;
}
const fullTree = buildSidebarTree({
	namespaces: namespaces.map((name) => ({ name, age: "1d" })),
	resourceKinds: extraKinds,
	argoDetected: false,
	fluxDetection: undefined,
	detectingGitOps: false,
	resourceKindsPending: false,
	resourceKindsError: "",
	showUnavailableGitOpsProviders: false,
	customResourceSearch: "operator-",
});

assert.equal(namespaceNodes.length, 1_000);
assert.equal(allExpandedChildren.length, 1_000);
assert.equal(
	resourceSets.size,
	namespaces.length - Math.ceil(namespaces.length / 97),
);
assert.equal(errorSets.size, Math.ceil(namespaces.length / 97));
assert.equal(pendingNamespaces.size > 0, true);
assert(
	lazyChildren.some((children) =>
		children.some((node) => node.label === "Loading custom resources..."),
	),
);
assert.deepEqual(
	fullTree.find((group) => group.id === "namespaces")?.nodes,
	namespaceNodes,
);
assert.equal(availableKinds.length, extraKinds.length + 2);
assert.equal(appendedKinds.length, extraKinds.length + 1);
assert.deepEqual(
	availableKinds.slice(2).map(discoveredKindIdentity),
	extraKinds.map(discoveredKindIdentity),
);
assert.deepEqual(fixedScopeKinds, ["Pod", "Deployment"]);
assert.equal(
	new Set(appendedKinds.map(discoveredKindIdentity)).size,
	appendedKinds.length,
);
assert(
	filterCustomResourceKinds(extraKinds, "wdg99").some(
		(kind) => kind.kind === "Widget99",
	),
);
assert(
	extraDiscoveredKinds(extraKinds).every(
		(kind, index, rows) =>
			index === 0 || (rows[index - 1]?.kind ?? "") <= kind.kind,
	),
);

describe("F26 lazy sidebar children with folded F27 query maps and F28 present kinds", () => {
	bench(
		"F26 eager custom-resource discovery tree roots (1k namespaces / 100 kinds)",
		() => {
			buildSidebarTree({
				namespaces: namespaces.map((name) => ({ name, age: "1d" })),
				resourceKinds: extraKinds,
				argoDetected: false,
				fluxDetection: undefined,
				detectingGitOps: false,
				resourceKindsPending: false,
				resourceKindsError: "",
				showUnavailableGitOpsProviders: false,
				customResourceSearch: "operator-",
			});
		},
	);

	bench("F26 shallow namespace nodes without collapsed children", () => {
		for (const namespace of namespaces)
			buildShallowNamespaceTreeNode(namespace);
	});

	bench("F26 one expanded namespace with all discovered kinds", () => {
		buildNamespaceTreeNode("namespace-0", extraKinds);
	});

	bench("F26 expanded children for per-namespace present-kind results", () => {
		for (const { namespace, kinds } of presentKindsByNamespace) {
			buildNamespaceTreeNode(namespace, kinds);
		}
	});

	bench(
		"F27 expanded result, error, and pending maps plus lazy child shaping",
		() => {
			const rows = customResourcesByExpandedNamespace(
				namespaces,
				namespaceQueryStates,
			);
			const errors = customResourceErrorsByExpandedNamespace(
				namespaces,
				namespaceQueryStates,
			);
			const pending = pendingCustomResourceNamespaces(
				namespaces,
				namespaceQueryStates,
			);
			for (const namespace of namespaces) {
				buildExpandedNamespaceChildren(
					namespace,
					rows.get(namespace) ?? [],
					errors.get(namespace),
					pending.has(namespace),
				);
			}
		},
	);

	bench(
		"F28 append present dynamic kinds and preserve a fixed-kind scope",
		() => {
			resourceBrowserAvailableKinds(
				["Pod", "Deployment"],
				duplicateDiscoveryObservations,
				false,
			);
			resourceBrowserAvailableKinds(["Pod", "Deployment"], extraKinds, true);
			appendPresentCustomResourceKinds(
				["Pod"],
				duplicateDiscoveryObservations,
			);
		},
	);
});
