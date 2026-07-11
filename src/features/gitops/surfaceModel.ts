import { normalizeArgoKindLabel } from "./gitops-nav";
import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	FluxResourceSummary,
} from "@/lib/types";
import type { TreeNodeId } from "@/lib/tree-nav";
import { gitOpsSelectionCells, type GitOpsSelection } from "./surfaceSelection";
export {
	gitOpsDetailsActionKey,
	gitOpsSelectionKey,
	gitOpsSelectionPrimaryAction,
	gitOpsSelectionResource,
} from "./surfaceSelection";
export type { GitOpsSelection } from "./surfaceSelection";
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
