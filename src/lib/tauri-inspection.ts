import { cancellableArg } from "./cancellable-loads";
import { kubeconfigArg, type TauriClient } from "./tauri";
import type {
	ArgoApplicationDetails,
	CancellableRequest,
	ArgoApplicationSetDetails,
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectDetails,
	ArgoAppProjectSummary,
	FluxDetectionSummary,
	FluxResourceDetails,
	FluxResourceKind,
	FluxResourceSummary,
	HelmReleaseDetails,
	HelmReleaseReconciliation,
	HelmReleaseSummary,
	IncidentCockpitSummary,
	RbacAccessReviewRequest,
	RbacAccessReviewResult,
	RbacInspectionSummary,
	ResourceListRequest,
	YamlEncoding,
	YamlViewMode,
} from "./types";

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

export async function detectFlux(
	client: TauriClient,
	clusterContext: string,
	kubeconfigEnvVar?: string,
): Promise<FluxDetectionSummary> {
	return client.invoke<FluxDetectionSummary>("detect_flux", {
		clusterContext,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function listFluxResources(
	client: TauriClient,
	clusterContext: string,
	resourceKind: FluxResourceKind,
	kubeconfigEnvVar?: string,
): Promise<FluxResourceSummary[]> {
	return client.invoke<FluxResourceSummary[]>("list_flux_resources", {
		clusterContext,
		resourceKind,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function getFluxResourceDetails(
	client: TauriClient,
	clusterContext: string,
	resourceKind: FluxResourceKind,
	name: string,
	namespace?: string | null,
	kubeconfigEnvVar?: string,
	yamlViewMode?: YamlViewMode,
	yamlEncoding?: YamlEncoding,
): Promise<FluxResourceDetails> {
	return client.invoke<FluxResourceDetails>("get_flux_resource_details", {
		clusterContext,
		resourceKind,
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
	kubeconfigEnvVar?: string,
	cancellable?: CancellableRequest,
): Promise<RbacInspectionSummary> {
	return client.invoke<RbacInspectionSummary>("list_rbac_inspection", {
		clusterContext,
		...kubeconfigArg(kubeconfigEnvVar),
		...cancellableArg(cancellable),
	});
}

export async function reviewRbacAccess(
	client: TauriClient,
	request: RbacAccessReviewRequest,
): Promise<RbacAccessReviewResult> {
	return client.invoke<RbacAccessReviewResult>("review_rbac_access", {
		requestId: request.requestId,
		cancelScope: request.cancelScope,
		request: {
			clusterContext: request.clusterContext,
			identity: request.identity,
			target: request.target,
			...kubeconfigArg(request.kubeconfigEnvVar),
		},
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
