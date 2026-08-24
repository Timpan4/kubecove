import { bench, describe } from "vitest";
import {
	buildNamespaceTreeNode,
	buildShallowNamespaceTreeNode,
} from "@/components/sidebar-tree-helpers";
import { buildSidebarTree } from "@/app/svelte/workspaceShellModel";
import type { DiscoveredResourceKind } from "@/lib/types";

const namespaces = Array.from({ length: 1_000 }, (_, index) => `namespace-${index}`);
const extraKinds: DiscoveredResourceKind[] = Array.from({ length: 100 }, (_, index) => ({
	group: `operator-${index % 40}.example.com`,
	version: "v1",
	apiVersion: `operator-${index % 40}.example.com/v1`,
	kind: `Widget${index}`,
	plural: `widgets${index}`,
	shortNames: [`wdg${index}`],
	namespaced: true,
}));

describe("sidebar tree", () => {
	bench("buildNamespaceTreeNode (eager, 1k namespaces x 100 kinds)", () => {
		for (const namespace of namespaces) {
			buildNamespaceTreeNode(namespace, extraKinds);
		}
	});

	bench("buildShallowNamespaceTreeNode (1k namespaces)", () => {
		for (const namespace of namespaces) {
			buildShallowNamespaceTreeNode(namespace);
		}
	});

	bench("buildSidebarTree (1k namespaces x 100 grouped and filtered kinds)", () => {
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
	});
});
