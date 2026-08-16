import {
	SECTIONS,
	STATIC_SECTION_NAMES,
	type SectionName,
	type TreeNodeId,
} from "@/lib/tree-nav";
import {
	CLUSTER_SCOPED_KINDS,
	SUPPORTED_KINDS,
	type DiscoveredResourceKind,
	type ResourceKindSelection,
	type ResourceSummary,
} from "@/lib/types";
import {
	buildFetchKeys,
	buildResourceSearchIndex,
	type FetchKey,
	type ResourceSearchEntry,
} from "@/features/resources/helpers";

export type PaletteAction = "settings" | "launcher";

export interface PaletteNavigationEntry {
	id: string;
	label: string;
	searchText: string;
	nodeId: TreeNodeId | null;
	action?: PaletteAction;
}

const ARGO_RESOURCE_KINDS: DiscoveredResourceKind[] = [
	{
		group: "argoproj.io",
		version: "v1alpha1",
		apiVersion: "argoproj.io/v1alpha1",
		kind: "Application",
		plural: "applications",
		namespaced: true,
	},
	{
		group: "argoproj.io",
		version: "v1alpha1",
		apiVersion: "argoproj.io/v1alpha1",
		kind: "ApplicationSet",
		plural: "applicationsets",
		namespaced: true,
	},
	{
		group: "argoproj.io",
		version: "v1alpha1",
		apiVersion: "argoproj.io/v1alpha1",
		kind: "AppProject",
		plural: "appprojects",
		namespaced: true,
	},
];

function resourceKindKey(kind: ResourceKindSelection): string {
	return typeof kind === "string"
		? `native:${kind}`
		: `dynamic:${kind.apiVersion}:${kind.plural}:${kind.kind}`;
}

function dedupeKinds(kinds: ResourceKindSelection[]): ResourceKindSelection[] {
	const seen = new Set<string>();
	return kinds.filter((kind) => {
		const key = resourceKindKey(kind);
		if (seen.has(key)) return false;
		seen.add(key);
		return true;
	});
}

export function buildGlobalSearchFetchKeys(
	namespaces: string[],
	customResourceKinds: DiscoveredResourceKind[],
	argoDetected: boolean,
	fluxResourceKinds: DiscoveredResourceKind[],
): FetchKey[] {
	const providerKinds = dedupeKinds([
		...(argoDetected ? ARGO_RESOURCE_KINDS : []),
		...fluxResourceKinds,
	]);
	const providerKeys = new Set(providerKinds.map(resourceKindKey));
	const scopedKinds = dedupeKinds([
		...SUPPORTED_KINDS,
		...CLUSTER_SCOPED_KINDS,
		...customResourceKinds.filter((kind) => !providerKeys.has(resourceKindKey(kind))),
	]);
	return [
		...buildFetchKeys(namespaces, scopedKinds),
		...buildFetchKeys([], providerKinds),
	];
}

function sectionEntry(section: SectionName): PaletteNavigationEntry {
	const label = SECTIONS[section].label;
	return {
		id: `section:${section}`,
		label,
		searchText: label.toLowerCase(),
		nodeId: { type: "section", section },
	};
}

function kindEntry(section: SectionName, child: string): PaletteNavigationEntry {
	const label = `${SECTIONS[section].label} › ${child}`;
	return {
		id: `kind:${section}:${child}`,
		label,
		searchText: label.toLowerCase(),
		nodeId: {
			type: "kind",
			section,
			namespace: undefined,
			group: undefined,
			kind: child,
		},
	};
}

/**
 * Static navigation targets: every sidebar section and its curated kind
 * children, plus the two top-bar destinations (Settings, Workspaces).
 */
export function buildNavigationEntries(gitOpsVisible: boolean): PaletteNavigationEntry[] {
	const entries: PaletteNavigationEntry[] = [];
	for (const section of STATIC_SECTION_NAMES) {
		if (section === "argo" && !gitOpsVisible) continue;
		entries.push(sectionEntry(section));
		for (const child of SECTIONS[section].children) {
			entries.push(kindEntry(section, child));
		}
	}
	entries.push({
		id: "action:settings",
		label: "Settings",
		searchText: "settings preferences",
		nodeId: null,
		action: "settings",
	});
	entries.push({
		id: "action:launcher",
		label: "Open Workspaces",
		searchText: "open workspaces launcher switch workspace",
		nodeId: null,
		action: "launcher",
	});
	return entries;
}

export function filterNavigationEntries(
	entries: PaletteNavigationEntry[],
	query: string,
): PaletteNavigationEntry[] {
	const term = query.trim().toLowerCase();
	if (!term) return entries;
	return entries.filter((entry) => entry.searchText.includes(term));
}

export function filterNamespaces(namespaces: string[], query: string): string[] {
	const term = query.trim().toLowerCase();
	if (!term) return namespaces;
	return namespaces.filter((namespace) =>
		namespace.toLowerCase().includes(term),
	);
}

export function resourceEntryKey(resource: ResourceSummary): string {
	return [
		resource.cluster,
		resource.apiVersion ?? "",
		resource.kind,
		resource.namespace ?? "",
		resource.name,
	].join("::");
}

export function dedupeResources(rows: ResourceSummary[]): ResourceSummary[] {
	const seen = new Set<string>();
	const result: ResourceSummary[] = [];
	for (const row of rows) {
		const key = resourceEntryKey(row);
		if (seen.has(key)) continue;
		seen.add(key);
		result.push(row);
	}
	return result;
}

export function buildDedupedResourceSearchIndex(
	resourceSets: ResourceSummary[][],
): ResourceSearchEntry[] {
	const seen = new Set<string>();
	const unique: ResourceSummary[] = [];
	for (const rows of resourceSets) {
		for (const resource of rows) {
			const key = resourceEntryKey(resource);
			if (seen.has(key)) continue;
			seen.add(key);
			unique.push(resource);
		}
	}
	return buildResourceSearchIndex(unique);
}
