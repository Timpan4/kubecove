import {
	discoveredResourceKindKey,
	KIND_GROUPS,
	type KindGroupName,
	type TreeNode,
	type TreeNodeId,
} from "@/lib/tree-nav";
import type { DiscoveredResourceKind } from "@/lib/types";

export function buildShallowNamespaceTreeNode(namespace: string): TreeNode {
	return {
		id: { type: "namespace", section: "namespaces", namespace },
		label: namespace,
	};
}

export function buildCustomResourceGroupNodes({
	section,
	namespace,
	extraKinds,
}: {
	section: string;
	namespace?: string;
	extraKinds: DiscoveredResourceKind[];
}): TreeNode[] {
	const kindsByGroup = new Map<string, DiscoveredResourceKind[]>();
	for (const resourceKind of extraKinds) {
		const groupKinds = kindsByGroup.get(resourceKind.group) ?? [];
		groupKinds.push(resourceKind);
		kindsByGroup.set(resourceKind.group, groupKinds);
	}
	return [...kindsByGroup.entries()]
		.toSorted(([left], [right]) => left.localeCompare(right))
		.map(([group, kinds]) => ({
			id: { type: "group", section, namespace, group } as TreeNodeId,
			label: group,
			selectable: false,
			children: kinds
				.toSorted((left, right) => left.kind.localeCompare(right.kind))
				.map((resourceKind) => {
					const shortNames = resourceKind.shortNames ?? [];
					return {
						id: {
							type: "kind",
							section,
							namespace,
							group,
							kind: discoveredResourceKindKey(resourceKind),
							resourceKind,
						} as TreeNodeId,
						label: resourceKind.kind,
						description: `${resourceKind.apiVersion} / ${resourceKind.plural}${
							shortNames.length > 0
								? ` / ${shortNames.join(", ")}`
								: ""
						}`,
					};
				}),
		}));
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
			selectable: false,
			children: buildCustomResourceGroupNodes({
				section: "namespaces",
				namespace,
				extraKinds: namespaceDiscoveredKinds,
			}),
		});
	}

	return {
		id: { type: "namespace", section: "namespaces", namespace },
		label: namespace,
		children: groups,
	};
}
