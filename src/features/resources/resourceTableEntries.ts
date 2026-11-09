import { gitOpsOwnershipGroupLabel } from "@/lib/gitops-ownership-evidence";
import { resourceKey } from "@/lib/resource-identity";
import type { ResourceSummary } from "@/lib/types";
import { pageGitOpsGroupCounts, pageTypeGroupCounts } from "./grouping";
import {
	formatResourceTypeGroupLabel,
	resourceGroupCollapseKey,
	resourceTypeGroupCollapseKey,
} from "./helpers";
import type {
	ResourceTableEntry,
	ResourceTablePage,
} from "./resourceTableTypes";

export function buildResourceTableEntries(
	page: ResourceTablePage,
	collapsedGroups: Set<string>,
	selectedResource?: ResourceSummary | null,
): ResourceTableEntry[] {
	const selected =
		selectedResource && collapsedGroups.size
			? page.rowForResource(selectedResource)
			: undefined;
	const { pageRows, groupedByGitOps, multiContext } = page;
	const selectedGroup = selected
		? resourceGroupCollapseKey(selected, multiContext)
		: null;
	const selectedType = selected
		? resourceTypeGroupCollapseKey(selected, multiContext)
		: null;
	const pageGroups = groupedByGitOps
		? pageGitOpsGroupCounts(pageRows, true, multiContext)
		: new Map<string, number>();
	const pageTypeGroups = pageTypeGroupCounts(pageRows, true, multiContext);
	const entries: ResourceTableEntry[] = [];
	let previous: ResourceSummary | null = null;
	let previousGroupLabel: string | null = null;
	for (const resource of pageRows) {
		const groupLabel = gitOpsOwnershipGroupLabel(resource, multiContext);
		const typeLabel = formatResourceTypeGroupLabel(resource);
		const groupKey = resourceGroupCollapseKey(resource, multiContext);
		const typeKey = resourceTypeGroupCollapseKey(resource, multiContext);
		const groupCollapsed =
			groupedByGitOps &&
			groupKey !== selectedGroup &&
			collapsedGroups.has(groupKey);
		const typeCollapsed =
			typeKey !== selectedType && collapsedGroups.has(typeKey);
		const showGroup = groupedByGitOps && previousGroupLabel !== groupLabel;
		const showType =
			!previous ||
			(groupedByGitOps && previousGroupLabel !== groupLabel) ||
			formatResourceTypeGroupLabel(previous) !== typeLabel;
		if (showGroup)
			entries.push({
				type: "group",
				key: groupKey,
				label: groupLabel,
				count: pageGroups.get(groupLabel) ?? 0,
				collapsed: groupCollapsed,
			});
		if (showType && !groupCollapsed)
			entries.push({
				type: "type",
				key: typeKey,
				label: typeLabel,
				kind: resource.kind,
				count: pageTypeGroups.get(`${groupLabel}::${typeLabel}`) ?? 0,
				collapsed: typeCollapsed,
			});
		if (!groupCollapsed && !typeCollapsed)
			entries.push({ type: "resource", key: resourceKey(resource), resource });
		previous = resource;
		previousGroupLabel = groupLabel;
	}
	return entries;
}

/** Retain entries only for the current page, including unchanged group headers. */
export function createResourceTableEntryBuilder() {
	let previous = new Map<string, ResourceTableEntry>();
	let previousEntries: ResourceTableEntry[] = [];
	return (
		page: ResourceTablePage,
		collapsed: Set<string>,
		selected?: ResourceSummary | null,
	) => {
		const entries = buildResourceTableEntries(page, collapsed, selected);
		const next = new Map<string, ResourceTableEntry>();
		let unchanged = entries.length === previousEntries.length;
		for (let index = 0; index < entries.length; index++) {
			const entry = entries[index];
			const key = `${entry.type}:${entry.key}`;
			const old = previous.get(key);
			if (old && sameEntry(old, entry)) entries[index] = old;
			if (entries[index] !== previousEntries[index]) unchanged = false;
			next.set(key, entries[index]);
		}
		previous = next;
		if (!unchanged) previousEntries = entries;
		return previousEntries;
	};
}

function sameEntry(
	left: ResourceTableEntry,
	right: ResourceTableEntry,
): boolean {
	if (left.type === "resource" || right.type === "resource") {
		return (
			left.type === "resource" &&
			right.type === "resource" &&
			left.resource === right.resource
		);
	}
	return (
		left.type === right.type &&
		left.label === right.label &&
		left.count === right.count &&
		left.collapsed === right.collapsed &&
		(left.type !== "type" ||
			(right.type === "type" && left.kind === right.kind))
	);
}
