import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	FluxResourceSummary,
	ResourceSummary,
} from "@/lib/types";
import { argoApplicationResourceNamespaces } from "@/features/resources";

export type GitOpsSelection =
	| { type: "argoApp"; item: ArgoApplicationSummary }
	| { type: "argoAppSet"; item: ArgoApplicationSetSummary }
	| { type: "argoProject"; item: ArgoAppProjectSummary }
	| { type: "flux"; item: FluxResourceSummary };

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

function gitOpsHealth(status: string | null | undefined): ResourceSummary["health"] {
	if (!status) return "unknown";
	if (status === "Synced" || status === "Healthy" || status === "True" || status === "Active") return "healthy";
	if (status === "Degraded" || status === "Missing" || status === "False") return "degraded";
	if (status === "OutOfSync" || status === "Progressing" || status === "Unknown") return "attention";
	return "unknown";
}
