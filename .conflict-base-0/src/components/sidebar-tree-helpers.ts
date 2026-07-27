import type { DiscoveredResourceKind } from "@/lib/types";
import {
	discoveredResourceKindKey,
	KIND_GROUPS,
	type KindGroupName,
	type TreeNode,
	type TreeNodeId,
} from "@/lib/tree-nav";

export function buildShallowNamespaceTreeNode(namespace: string): TreeNode {
	return {
		id: { type: "namespace", section: "namespaces", namespace },
		label: namespace,
	};
}

export function buildNamespaceTreeNode(
	namespace: string,
	extraKinds: DiscoveredResourceKind[],
): TreeNode {
	const groups: TreeNode[] = (Object.keys(KIND_GROUPS) as KindGroupName[]).map(
		(groupName) => {
			const kinds = KIND_GROUPS[groupName];
			return {
				id: {
					type: "group",
					section: "namespaces",
					namespace,
					group: groupName,
				} as TreeNodeId,
				label: groupName,
				children: kinds.map((kind) => ({
					id: {
						type: "kind",
						section: "namespaces",
						namespace,
						group: groupName,
						kind,
					} as TreeNodeId,
					label: kind,
				})),
			};
		},
	);
	const namespaceDiscoveredKinds = extraKinds.filter(
		(resourceKind) => resourceKind.namespaced,
	);
	if (namespaceDiscoveredKinds.length > 0) {
		groups.push({
			id: {
				type: "group",
				section: "namespaces",
				namespace,
				group: "Custom Resources",
			} as TreeNodeId,
			label: "Custom Resources",
			children: namespaceDiscoveredKinds.map((resourceKind) => ({
				id: {
					type: "kind",
					section: "namespaces",
					namespace,
					group: "Custom Resources",
					kind: discoveredResourceKindKey(resourceKind),
					resourceKind,
				} as TreeNodeId,
				label: resourceKind.kind,
				description: `${resourceKind.apiVersion} / ${resourceKind.plural}`,
			})),
		});
	}

	return {
		id: { type: "namespace", section: "namespaces", namespace },
		label: namespace,
		children: groups,
	};
}
