import {
	discoveredResourceKindKey,
	KIND_GROUPS,
	type KindGroupName,
	type TreeNode,
} from "@/lib/tree-nav";
import type { DiscoveredResourceKind } from "@/lib/types";

export function buildShallowNamespaceTreeNode(namespace: string): TreeNode {
	return {
		id: { type: "namespace", section: "namespaces", namespace },
		label: namespace,
	};
}

export function extraDiscoveredKinds(
	resourceKinds: DiscoveredResourceKind[],
): DiscoveredResourceKind[] {
	return resourceKinds
		.toSorted((left, right) => {
			return (
				left.kind.localeCompare(right.kind) ||
				left.apiVersion.localeCompare(right.apiVersion) ||
				left.plural.localeCompare(right.plural)
			);
		});
}

export function filterCustomResourceKinds(
	resourceKinds: DiscoveredResourceKind[],
	search: string,
): DiscoveredResourceKind[] {
	const query = search.trim().toLowerCase();
	if (!query) return resourceKinds;
	return resourceKinds.filter((resourceKind) =>
		[
			resourceKind.kind,
			resourceKind.plural,
			...(resourceKind.shortNames ?? []),
			resourceKind.group,
			resourceKind.apiVersion,
		].some((value) => value.toLowerCase().includes(query)),
	);
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
		.map(([group, kinds]): TreeNode => ({
			id: { type: "group", section, namespace, group },
			label: group,
			selectable: false,
			children: kinds
				.toSorted((left, right) => left.kind.localeCompare(right.kind))
				.map((resourceKind): TreeNode => {
					const shortNames = resourceKind.shortNames ?? [];
					return {
						id: {
							type: "kind",
							section,
							namespace,
							group,
							kind: discoveredResourceKindKey(resourceKind),
							resourceKind,
						},
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
	const kindGroupNames = Object.keys(KIND_GROUPS).filter(
		(groupName): groupName is KindGroupName => groupName in KIND_GROUPS,
	);
	const groups: TreeNode[] = kindGroupNames.map(
		(groupName) => {
			const kinds = KIND_GROUPS[groupName];
			return {
				id: {
					type: "group",
					section: "namespaces",
					namespace,
					group: groupName,
				},
				label: groupName,
				children: kinds.map((kind): TreeNode => ({
					id: {
						type: "kind",
						section: "namespaces",
						namespace,
						group: groupName,
						kind,
					},
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
			},
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
