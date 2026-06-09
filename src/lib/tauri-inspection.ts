import type { TauriClient } from "./tauri";
import type {
	ArgoApplicationDetails,
	ArgoApplicationSetDetails,
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectDetails,
	ArgoAppProjectSummary,
	HelmReleaseDetails,
	HelmReleaseReconciliation,
	HelmReleaseSummary,
	IncidentCockpitSummary,
	RbacInspectionSummary,
	ResourceListRequest,
	YamlEncoding,
	YamlViewMode,
} from "./types";

function kubeconfigArg(kubeconfigEnvVar?: string): {
	kubeconfigEnvVar?: string;
} {
	if (
		kubeconfigEnvVar === undefined ||
		kubeconfigEnvVar.startsWith("kubeconfigSource=")
	) {
		return {};
	}
	return { kubeconfigEnvVar };
}

export async function detectArgoCD(
	client: TauriClient,
	clusterContext: string,
	kubeconfigEnvVar?: string,
): Promise<boolean> {
	return client.invoke<boolean>("detect_argocd", {
		clusterContext,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function listArgoApplications(
	client: TauriClient,
	clusterContext: string,
	kubeconfigEnvVar?: string,
): Promise<ArgoApplicationSummary[]> {
	return client.invoke<ArgoApplicationSummary[]>("list_argocd_applications", {
		clusterContext,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function getArgoApplicationDetails(
	client: TauriClient,
	clusterContext: string,
	name: string,
	namespace?: string,
	kubeconfigEnvVar?: string,
	yamlViewMode?: YamlViewMode,
	yamlEncoding?: YamlEncoding,
): Promise<ArgoApplicationDetails> {
	return client.invoke<ArgoApplicationDetails>(
		"get_argocd_application_details",
		{
			clusterContext,
			name,
			namespace,
			...kubeconfigArg(kubeconfigEnvVar),
			yamlViewMode,
			yamlEncoding,
		},
	);
}

export async function listArgoApplicationSets(
	client: TauriClient,
	clusterContext: string,
	kubeconfigEnvVar?: string,
): Promise<ArgoApplicationSetSummary[]> {
	return client.invoke<ArgoApplicationSetSummary[]>("list_argocd_appsets", {
		clusterContext,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function listArgoAppProjects(
	client: TauriClient,
	clusterContext: string,
	kubeconfigEnvVar?: string,
): Promise<ArgoAppProjectSummary[]> {
	return client.invoke<ArgoAppProjectSummary[]>("list_argocd_appprojects", {
		clusterContext,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function getArgoApplicationSetDetails(
	client: TauriClient,
	clusterContext: string,
	name: string,
	namespace?: string,
	kubeconfigEnvVar?: string,
	yamlViewMode?: YamlViewMode,
	yamlEncoding?: YamlEncoding,
): Promise<ArgoApplicationSetDetails> {
	return client.invoke<ArgoApplicationSetDetails>("get_argocd_appset_details", {
		clusterContext,
		name,
		namespace,
		...kubeconfigArg(kubeconfigEnvVar),
		yamlViewMode,
		yamlEncoding,
	});
}

export async function getArgoAppProjectDetails(
	client: TauriClient,
	clusterContext: string,
	name: string,
	namespace?: string,
	kubeconfigEnvVar?: string,
	yamlViewMode?: YamlViewMode,
	yamlEncoding?: YamlEncoding,
): Promise<ArgoAppProjectDetails> {
	return client.invoke<ArgoAppProjectDetails>("get_argocd_appproject_details", {
		clusterContext,
		name,
		namespace,
		...kubeconfigArg(kubeconfigEnvVar),
		yamlViewMode,
		yamlEncoding,
	});
}

export async function listHelmReleases(
	client: TauriClient,
	clusterContext: string,
	kubeconfigEnvVar?: string,
): Promise<HelmReleaseSummary[]> {
	return client.invoke<HelmReleaseSummary[]>("list_helm_releases", {
		clusterContext,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function getHelmReleaseDetails(
	client: TauriClient,
	release: Pick<
		HelmReleaseSummary,
		"cluster" | "namespace" | "storageKind" | "storageName"
	>,
	kubeconfigEnvVar?: string,
	yamlViewMode?: YamlViewMode,
	yamlEncoding?: YamlEncoding,
): Promise<HelmReleaseDetails> {
	return client.invoke<HelmReleaseDetails>("get_helm_release_details", {
		clusterContext: release.cluster,
		namespace: release.namespace,
		storageKind: release.storageKind,
		storageName: release.storageName,
		...kubeconfigArg(kubeconfigEnvVar),
		yamlViewMode,
		yamlEncoding,
	});
}

export async function getHelmReleaseReconciliation(
	client: TauriClient,
	release: Pick<
		HelmReleaseSummary,
		"cluster" | "namespace" | "storageKind" | "storageName"
	>,
	kubeconfigEnvVar?: string,
): Promise<HelmReleaseReconciliation> {
	return client.invoke<HelmReleaseReconciliation>(
		"get_helm_release_reconciliation",
		{
			clusterContext: release.cluster,
			namespace: release.namespace,
			storageKind: release.storageKind,
			storageName: release.storageName,
			...kubeconfigArg(kubeconfigEnvVar),
		},
	);
}

export async function listRbacInspection(
	client: TauriClient,
	clusterContext: string,
	namespaces: string[],
	kubeconfigEnvVar?: string,
): Promise<RbacInspectionSummary> {
	return client.invoke<RbacInspectionSummary>("list_rbac_inspection", {
		clusterContext,
		namespaces,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function listIncidentCockpit(
	client: TauriClient,
	clusterContext: string,
	requests: ResourceListRequest[],
	kubeconfigEnvVar?: string,
): Promise<IncidentCockpitSummary> {
	return client.invoke<IncidentCockpitSummary>("list_incident_cockpit", {
		clusterContext,
		requests,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}
