import { bench, describe } from "vitest";
import {
	buildNamespaceTreeNode,
	buildShallowNamespaceTreeNode,
} from "@/components/sidebar-tree-helpers";
import type { DiscoveredResourceKind } from "@/lib/types";

const namespaces = Array.from({ length: 1_000 }, (_, index) => `namespace-${index}`);
const extraKinds: DiscoveredResourceKind[] = Array.from({ length: 100 }, (_, index) => ({
	group: "example.com",
	version: "v1",
	apiVersion: "example.com/v1",
	kind: `Widget${index}`,
	plural: `widgets${index}`,
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
});
