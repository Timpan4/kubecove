import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	FluxResourceSummary,
	ResourceSummary,
} from "@/lib/types";

export type GitOpsSelection =
	| { type: "argoApp"; item: ArgoApplicationSummary }
	| { type: "argoAppSet"; item: ArgoApplicationSetSummary }
	| { type: "argoProject"; item: ArgoAppProjectSummary }
	| { type: "flux"; item: FluxResourceSummary };

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
			healthAssessment: item.healthAssessment,
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
		healthAssessment: item.healthAssessment,
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
