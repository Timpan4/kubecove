// Exports the browser-mock responses the GitOps and Helm pages read: Argo CD
// ApplicationSets and AppProjects, each Application's resource details and its
// Kubernetes-transport inspector snapshot, and Helm releases with their details
// and reconciliation. Shapes follow the Rust models the GPUI fixture loads.
import type { TauriClient } from "../../../src/lib/tauri";
import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	HealthAssessment,
	HelmReleaseDetails,
	HelmReleaseList,
	HelmReleaseReconciliation,
	ResourceDetailsFull,
	ResourceSummary,
	VersionedList,
} from "../../../src/lib/types";

// The mocks omit the assessment the Rust ApplicationSet and AppProject models
// require; the web counts a missing assessment as unknown.
const UNKNOWN: HealthAssessment = { state: "unknown", completeness: "complete", winningSources: [], reasons: [], evidence: [] };

function withAssessment<T extends { healthAssessment?: HealthAssessment }>(item: T): T & { healthAssessment: HealthAssessment } {
	return { ...item, healthAssessment: item.healthAssessment ?? UNKNOWN };
}

// Browser mocks omit owner fields the Rust model requires, as in `export-fixture.ts`.
function withRustOwnerFields(row: ResourceSummary): ResourceSummary {
	const owner = row.gitOpsOwner;
	return owner ? { ...row, gitOpsOwner: { provenance: "browser-mock", ...owner, partial: owner.partial ?? owner.confidence !== "inventory" } } : row;
}

/** The kind `gitOpsSelectionResource` gives an Application row, which the Details button inspects. */
const APPLICATION_KIND = { kind: "Application", apiVersion: "argoproj.io/v1alpha1", group: "argoproj.io", version: "v1alpha1", plural: "applications", namespaced: true };

export async function exportGitOpsPage(client: TauriClient, clusterContext: string | undefined, argoDetected: boolean) {
	if (!argoDetected) return { appSets: [], projects: [], applications: [] };
	// The fixture keeps rows only; its reads report the mock list version themselves.
	const appSets = (await client.invoke<VersionedList<ArgoApplicationSetSummary>>("list_argocd_appsets", { clusterContext })).items.map(withAssessment);
	const projects = (await client.invoke<VersionedList<ArgoAppProjectSummary>>("list_argocd_appprojects", { clusterContext })).items.map(withAssessment);
	const apps = (await client.invoke<VersionedList<ArgoApplicationSummary>>("list_argocd_applications", { clusterContext })).items;
	const applications = [];
	for (const app of apps) {
		const details = await client.invoke<ResourceDetailsFull>("get_dynamic_resource_details", { clusterContext, resourceKind: APPLICATION_KIND, name: app.name, namespace: app.namespace });
		const application = { name: app.name, namespace: app.namespace ?? null, project: app.project ?? null, resourceVersion: null, uid: null, apiVersion: APPLICATION_KIND.apiVersion, context: clusterContext, workspaceId: null };
		const inspector = await client.invoke("get_argo_application_inspector", { clusterContext, transport: "kubernetes", application });
		applications.push({ details: { ...details, summary: withRustOwnerFields(details.summary) }, inspector });
	}
	return { appSets, projects, applications };
}

export async function exportHelm(client: TauriClient, clusterContext: string | undefined) {
	const list = await client.invoke<HelmReleaseList>("list_helm_releases", { clusterContext });
	const details: HelmReleaseDetails[] = [];
	const reconciliations: HelmReleaseReconciliation[] = [];
	for (const release of list.releases) {
		const target = { clusterContext: release.cluster, namespace: release.namespace, storageKind: release.storageKind, storageName: release.storageName };
		details.push(await client.invoke<HelmReleaseDetails>("get_helm_release_details", target));
		const reconciliation = await client.invoke<HelmReleaseReconciliation>("get_helm_release_reconciliation", target);
		reconciliations.push({ ...reconciliation, resources: reconciliation.resources.map((resource) => (resource.liveResource ? { ...resource, liveResource: withRustOwnerFields(resource.liveResource) } : resource)) });
	}
	return { list, details, reconciliations };
}
