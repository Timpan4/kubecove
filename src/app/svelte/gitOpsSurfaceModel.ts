import { normalizeArgoKindLabel } from "@/features/gitops/gitops-nav";
import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	FluxResourceSummary,
} from "@/lib/types";
import type { TreeNodeId } from "@/lib/tree-nav";

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

export function gitOpsUnavailableProvider(
	data: GitOpsData,
	selectedNode: TreeNodeId | null,
): { title: string; description: string } | null {
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
