import { normalizeArgoKindLabel } from "./gitops-nav";
import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	FluxResourceSummary,
	ResourceSummary,
} from "@/lib/types";
import type { TreeNodeId } from "@/lib/tree-nav";
export {
	gitOpsSelectionAgeTooltip,
	gitOpsSelectionRevisionLabel,
	gitOpsSelectionRevisionTooltipLines,
	gitOpsSelectionRevisionTooltipRows,
	gitOpsSelectionRevisionTooltipTitle,
	gitOpsSelectionSourceLabel,
	gitOpsSelectionSourceLine,
	gitOpsSelectionSourceMode,
	gitOpsSelectionSourceTooltip,
	gitOpsSelectionSourceTooltipGroups,
	gitOpsSelectionSourceTooltipLines,
	gitOpsSelectionSourceTooltipTitle,
} from "./surfaceTooltips";
export type {
	GitOpsRevisionTooltipRow,
	GitOpsSourceMode,
	GitOpsSourceTooltipGroup,
	GitOpsSourceTooltipRow,
	GitOpsTooltipField,
} from "./surfaceTooltips";

export interface GitOpsData {
	argoDetected?: boolean;
	apps: ArgoApplicationSummary[];
	appSets: ArgoApplicationSetSummary[];
	projects: ArgoAppProjectSummary[];
	flux: FluxResourceSummary[];
	fluxDetected?: boolean;
}

export interface GitOpsTable {
	title: string;
	headers: string[];
	rows: string[][];
	empty: string;
}

export interface GitOpsRailItem {
	key: string;
	provider: "Argo CD" | "Flux";
	label: string;
	count: number;
	disabled?: boolean;
}

export interface GitOpsUnavailableProvider {
	title: string;
	description: string;
}

export type GitOpsSelection =
	| { type: "argoApp"; item: ArgoApplicationSummary }
	| { type: "argoAppSet"; item: ArgoApplicationSetSummary }
	| { type: "argoProject"; item: ArgoAppProjectSummary }
	| { type: "flux"; item: FluxResourceSummary };

export function buildGitOpsTable(
	data: GitOpsData,
	selectedNode: TreeNodeId | null,
): GitOpsTable {
	const selectedKind = selectedNode?.type === "kind" ? selectedNode.kind : null;
	const argoKind = normalizeArgoKindLabel(selectedKind ?? null);
	if (argoKind === "applicationSets") {
		return {
			title: "Argo CD ApplicationSets",
			headers: ["Name", "Namespace", "Sync", "Health", "Project", "Source"],
			rows: buildGitOpsSelections(data, selectedNode).map(gitOpsSelectionCells),
			empty: "No Argo CD ApplicationSets found",
		};
	}
	if (argoKind === "appProjects") {
		return {
			title: "Argo CD AppProjects",
			headers: ["Name", "Namespace", "Status", "Description", "Age"],
			rows: buildGitOpsSelections(data, selectedNode).map(gitOpsSelectionCells),
			empty: "No Argo CD AppProjects found",
		};
	}
	if (isFluxSelection(selectedNode)) {
		return {
			title: selectedKind ? `Flux ${selectedKind}` : "Flux Resources",
			headers: ["Resource", "Namespace", "Ready", "Source", "Revision", "Message"],
			rows: buildGitOpsSelections(data, selectedNode).map(gitOpsSelectionCells),
			empty: "No Flux resources found",
		};
	}
	if (shouldShowFluxByDefault(data, selectedNode)) {
		return {
			title: "Flux Resources",
			headers: ["Resource", "Namespace", "Ready", "Source", "Revision", "Message"],
			rows: buildGitOpsSelections(data, selectedNode).map(gitOpsSelectionCells),
			empty: "No Flux resources found",
		};
	}
	return {
		title: "Argo CD Applications",
		headers: ["Name", "Namespace", "Sync", "Health", "Destination", "Source"],
		rows: buildGitOpsSelections(data, selectedNode).map(gitOpsSelectionCells),
		empty: "No Argo CD Applications found",
	};
}

export function buildGitOpsSelections(
	data: GitOpsData,
	selectedNode: TreeNodeId | null,
): GitOpsSelection[] {
	const selectedKind = selectedNode?.type === "kind" ? selectedNode.kind : null;
	const argoKind = normalizeArgoKindLabel(selectedKind ?? null);
	if (argoKind === "applicationSets") {
		return data.appSets.map((item) => ({ type: "argoAppSet", item }));
	}
	if (argoKind === "appProjects") {
		return data.projects.map((item) => ({ type: "argoProject", item }));
	}
	if (isFluxSelection(selectedNode)) {
		return data.flux
			.filter((item) => !selectedKind || item.resourceKind.kind === selectedKind)
			.map((item) => ({ type: "flux", item }));
	}
	if (shouldShowFluxByDefault(data, selectedNode)) {
		return data.flux.map((item) => ({ type: "flux", item }));
	}
	return data.apps.map((item) => ({ type: "argoApp", item }));
}

export function buildGitOpsRailItems(data: GitOpsData): GitOpsRailItem[] {
	const items: GitOpsRailItem[] = [
		{
			key: "argo:applications",
			provider: "Argo CD",
			label: "Applications",
			count: data.apps.length,
			disabled: data.argoDetected === false,
		},
		{
			key: "argo:applicationSets",
			provider: "Argo CD",
			label: "ApplicationSets",
			count: data.appSets.length,
			disabled: data.argoDetected === false,
		},
		{
			key: "argo:appProjects",
			provider: "Argo CD",
			label: "AppProjects",
			count: data.projects.length,
			disabled: data.argoDetected === false,
		},
	];

	if (data.fluxDetected || data.flux.length > 0) {
		const fluxCounts = new Map<string, number>();
		for (const resource of data.flux) {
			fluxCounts.set(
				resource.resourceKind.kind,
				(fluxCounts.get(resource.resourceKind.kind) ?? 0) + 1,
			);
		}
		if (fluxCounts.size === 0) {
			items.push({
				key: "flux:resources",
				provider: "Flux",
				label: "Resources",
				count: 0,
				disabled: data.fluxDetected === false,
			});
		} else {
			for (const [kind, count] of [...fluxCounts.entries()].sort(([left], [right]) =>
				left.localeCompare(right),
			)) {
				items.push({
					key: `flux:${kind}`,
					provider: "Flux",
					label: kind,
					count,
					disabled: data.fluxDetected === false,
				});
			}
		}
	}

	return items;
}

export function gitOpsActiveRailKey(
	data: GitOpsData,
	selectedNode: TreeNodeId | null,
): string {
	const selectedKind = selectedNode?.type === "kind" ? selectedNode.kind : null;
	const argoKind = normalizeArgoKindLabel(selectedKind ?? null);
	if (argoKind === "applicationSets") return "argo:applicationSets";
	if (argoKind === "appProjects") return "argo:appProjects";
	if (isFluxSelection(selectedNode)) return selectedKind ? `flux:${selectedKind}` : "flux:resources";
	if (shouldShowFluxByDefault(data, selectedNode)) {
		return data.flux[0] ? `flux:${data.flux[0].resourceKind.kind}` : "flux:resources";
	}
	return "argo:applications";
}

export function gitOpsSelectionPrimaryAction(
	selection: GitOpsSelection,
): "openResources" | "details" {
	return selection.type === "argoApp" ? "openResources" : "details";
}

export function gitOpsDetailsActionKey(selection: GitOpsSelection): string {
	return `details:${gitOpsSelectionKey(selection)}`;
}

export function gitOpsSelectionResource(selection: GitOpsSelection): ResourceSummary {
	if (selection.type === "flux") {
		const { item } = selection;
		return {
			kind: item.resourceKind.kind,
			cluster: item.cluster,
			name: item.name,
			namespace: item.namespace,
			age: item.age,
			createdAt: item.createdAt,
			apiVersion: item.resourceKind.apiVersion,
			group: item.resourceKind.group,
			version: item.resourceKind.version,
			plural: item.resourceKind.plural,
			namespaced: item.resourceKind.namespaced,
			dynamic: true,
			health: gitOpsHealth(item.readyStatus),
			status: item.readyStatus,
			ready: item.readyStatus,
			gitOpsOwner: {
				provider: "flux",
				kind: item.resourceKind.kind,
				name: item.name,
				namespace: item.namespace,
				confidence: "metadata",
			},
		};
	}
	if (selection.type === "argoApp") {
		return argoResourceSummary(selection.item, {
			kind: "Application",
			plural: "applications",
			status: selection.item.healthStatus ?? selection.item.syncStatus,
			health: selection.item.healthStatus ?? selection.item.syncStatus,
		});
	}
	if (selection.type === "argoAppSet") {
		return argoResourceSummary(selection.item, {
			kind: "ApplicationSet",
			plural: "applicationsets",
			status: selection.item.status ?? selection.item.healthStatus ?? selection.item.syncStatus,
			health: selection.item.healthStatus ?? selection.item.syncStatus ?? selection.item.status,
		});
	}
	return argoResourceSummary(selection.item, {
		kind: "AppProject",
		plural: "appprojects",
		status: selection.item.status,
		health: selection.item.status,
	});
}

function argoResourceSummary(
	item: ArgoApplicationSummary | ArgoApplicationSetSummary | ArgoAppProjectSummary,
	argoKind: {
		kind: "Application" | "ApplicationSet" | "AppProject";
		plural: "applications" | "applicationsets" | "appprojects";
		status?: string | null;
		health?: string | null;
	},
): ResourceSummary {
	return {
		kind: argoKind.kind,
		cluster: item.cluster,
		name: item.name,
		namespace: item.namespace,
		age: item.age,
		createdAt: item.createdAt,
		apiVersion: "argoproj.io/v1alpha1",
		group: "argoproj.io",
		version: "v1alpha1",
		plural: argoKind.plural,
		namespaced: true,
		dynamic: true,
		health: gitOpsHealth(argoKind.health),
		status: argoKind.status ?? undefined,
		ready: argoKind.status ?? undefined,
		gitOpsOwner: {
			provider: "argo",
			kind: argoKind.kind,
			name: item.name,
			namespace: item.namespace,
			confidence: "metadata",
		},
	};
}

export function gitOpsUnavailableProvider(
	data: GitOpsData,
	selectedNode: TreeNodeId | null,
): GitOpsUnavailableProvider | null {
	if (selectedNode?.type !== "group" || selectedNode.section !== "argo") return null;
	if (selectedNode.group === "gitops:argo" && data.argoDetected === false) {
		return {
			title: "Argo CD not detected",
			description: "Argo CD CRDs were not detected in this cluster.",
		};
	}
	if (selectedNode.group === "gitops:flux" && data.fluxDetected === false) {
		return {
			title: "Flux not detected",
			description: "Flux CRDs were not detected in this cluster.",
		};
	}
	return null;
}

export function gitOpsSelectionKey(selection: GitOpsSelection): string {
	if (selection.type === "flux") {
		const { item } = selection;
		return [
			"flux",
			item.cluster,
			item.resourceKind.apiVersion,
			item.resourceKind.plural,
			item.namespace ?? "",
			item.name,
		].join(":");
	}
	const { item } = selection;
	return [selection.type, item.cluster, item.namespace ?? "", item.name].join(":");
}

export function gitOpsSelectionCells(selection: GitOpsSelection): string[] {
	if (selection.type === "argoAppSet") {
		const { item } = selection;
		return [
			item.name,
			item.namespace ?? "-",
			item.syncStatus ?? "-",
			item.healthStatus ?? "-",
			item.project ?? "-",
			item.sourceRepo ?? "-",
		];
	}
	if (selection.type === "argoProject") {
		const { item } = selection;
		return [
			item.name,
			item.namespace ?? "-",
			item.status ?? "-",
			item.description ?? "-",
			item.age,
		];
	}
	if (selection.type === "flux") {
		const { item } = selection;
		return [
			`${item.resourceKind.kind}/${item.name}`,
			item.namespace ?? "-",
			item.readyStatus ?? "-",
			item.sourceName ?? "-",
			item.lastAppliedRevision ?? "-",
			item.message ?? "-",
		];
	}
	const { item } = selection;
	return [
		item.name,
		item.namespace ?? "-",
		item.syncStatus ?? "-",
		item.healthStatus ?? "-",
		item.destinationNamespace ?? item.destinationServer ?? "-",
		item.sourceRepo ?? "-",
	];
}

function isFluxSelection(selectedNode: TreeNodeId | null): boolean {
	return (
		selectedNode?.section === "argo" &&
		typeof selectedNode.group === "string" &&
		selectedNode.group.startsWith("gitops:flux")
	);
}

function isArgoSelection(selectedNode: TreeNodeId | null): boolean {
	return selectedNode?.section === "argo" && selectedNode.group === "gitops:argo";
}

function shouldShowFluxByDefault(
	data: GitOpsData,
	selectedNode: TreeNodeId | null,
): boolean {
	return !isArgoSelection(selectedNode) && data.argoDetected === false && data.flux.length > 0;
}

function gitOpsHealth(status: string | null | undefined): ResourceSummary["health"] {
	if (!status) return "unknown";
	if (status === "Synced" || status === "Healthy" || status === "True" || status === "Active") {
		return "healthy";
	}
	if (status === "Degraded" || status === "Missing" || status === "False") return "degraded";
	if (status === "OutOfSync" || status === "Progressing" || status === "Unknown") {
		return "attention";
	}
	return "unknown";
}
