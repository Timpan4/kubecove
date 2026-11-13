import { CircleHelp, GitBranch, Layers, Package, Plug } from "lucide-svelte";
import {
	type GitOpsSelection,
	type GitOpsSourceMode,
	gitOpsSelectionPrimaryAction,
	gitOpsSelectionResource,
} from "./surfaceModel";

/** Presentation shared by the GitOps cards and the GitOps list. */

export function gitOpsCardActionLabel(selection: GitOpsSelection): string {
	if (gitOpsSelectionPrimaryAction(selection) === "openResources") {
		return `View resources for ${selection.item.name}`;
	}
	return `Open details for ${selection.item.name}`;
}

export function gitOpsDetailsActionLabel(selection: GitOpsSelection): string {
	const resource = gitOpsSelectionResource(selection);
	return `Open details for ${resource.kind} ${resource.name} ${resource.namespace ? `in ${resource.namespace}` : "at cluster scope"}`;
}

export function stopTooltipEvent(event: KeyboardEvent) {
	if (event.key !== "Escape") event.stopPropagation();
}

export function gitOpsCardSubtitle(selection: GitOpsSelection): string {
	if (selection.type === "flux") return selection.item.resourceKind.kind;
	if (selection.type === "argoProject") return selection.item.description ?? "AppProject";
	return selection.item.project ?? "default";
}

/** Raw provider statuses, labelled. `trackedOperation` is the last request KubeCove sent for an Application. */
export function gitOpsCardBadges(
	selection: GitOpsSelection,
	trackedOperation: string | null,
): [string, string][] {
	if (selection.type === "flux") {
		return selection.item.readyStatus ? [["Ready", selection.item.readyStatus]] : [];
	}
	if (selection.type === "argoProject") {
		return selection.item.status ? [["Status", selection.item.status]] : [];
	}
	return [
		["Last request", trackedOperation],
		["Operation", selection.type === "argoApp" ? selection.item.refreshRequested ? "Refreshing" : selection.item.operationPhase : null],
		["Sync", selection.item.syncStatus],
		["Health", selection.item.healthStatus],
	].filter((entry): entry is [string, string] => Boolean(entry[1]));
}

export function gitOpsCardTone(selection: GitOpsSelection): string {
	const state = selection.item.healthAssessment?.state;
	if (state === "degraded") return "bg-destructive";
	if (state === "needsAttention") return "bg-amber-400";
	if (state === "healthy") return "bg-emerald-400";
	return "bg-muted";
}

export function gitOpsSourceIcon(sourceMode: GitOpsSourceMode | null) {
	if (sourceMode === "git") return GitBranch;
	if (sourceMode === "helm") return Package;
	if (sourceMode === "multi") return Layers;
	if (sourceMode === "plugin") return Plug;
	if (sourceMode === "unknown") return CircleHelp;
	return null;
}

export function argoSourceCount(selection: GitOpsSelection): number | undefined {
	if (selection.type !== "argoApp") return undefined;
	const count = selection.item.sourceCount ?? selection.item.sources?.length;
	return count && count > 1 ? count : undefined;
}
