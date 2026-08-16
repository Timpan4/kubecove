import type {
	GitOpsSelection,
} from "@/lib/gitops-resource";

export type { GitOpsSelection } from "@/lib/gitops-resource";
export { gitOpsSelectionResource } from "@/lib/gitops-resource";

import { argoApplicationResourceNamespaces } from "@/features/resources";

export function selectedGitOpsApplicationName(selection: GitOpsSelection | null): string | null {
	return selection?.type === "argoApp" ? selection.item.name : null;
}

export function resolveTargetGitOpsSelection(
	selections: GitOpsSelection[],
	targetApplication: string | null | undefined,
	dataReady: boolean,
): { selection: GitOpsSelection | null; shouldResolve: boolean } {
	if (!targetApplication) return { selection: null, shouldResolve: false };
	const selection =
		selections.find(
			(item) => item.type === "argoApp" && item.item.name === targetApplication,
		) ?? null;
	return { selection, shouldResolve: Boolean(selection) || dataReady };
}

export function argoApplicationResourceNavigation(selection: GitOpsSelection) {
	if (selection.type !== "argoApp") return null;
	return {
		namespaces: argoApplicationResourceNamespaces(selection.item),
		gitOpsFilter: "",
		healthFilter: "all" as const,
		focusApplication: selection.item,
	};
}

export function gitOpsSelectionPrimaryAction(
	selection: GitOpsSelection,
): "openResources" | "details" {
	return selection.type === "argoApp" ? "openResources" : "details";
}

export function gitOpsDetailsActionKey(selection: GitOpsSelection): string {
	return `details:${gitOpsSelectionKey(selection)}`;
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
		return [item.name, item.namespace ?? "-", item.syncStatus ?? "-", item.healthStatus ?? "-", item.project ?? "-", item.sourceRepo ?? "-"];
	}
	if (selection.type === "argoProject") {
		const { item } = selection;
		return [item.name, item.namespace ?? "-", item.status ?? "-", item.description ?? "-", item.age];
	}
	if (selection.type === "flux") {
		const { item } = selection;
		return [`${item.resourceKind.kind}/${item.name}`, item.namespace ?? "-", item.readyStatus ?? "-", item.sourceName ?? "-", item.lastAppliedRevision ?? "-", item.message ?? "-"];
	}
	const { item } = selection;
	return [item.name, item.namespace ?? "-", item.syncStatus ?? "-", item.healthStatus ?? "-", item.destinationNamespace ?? item.destinationServer ?? "-", item.sourceRepo ?? "-"];
}
