import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { YamlCodeViewer } from "@/components/YamlCodeViewer";
import { kubeconfigSourceKey, useSettingsState } from "@/lib/settings";
import { createTauriClient, getArgoAppProjectDetails } from "@/lib/tauri";
import type {
	ArgoAppProjectSummary,
	YamlEncoding,
	YamlViewMode,
} from "@/lib/types";
import {
	DetailErrorState,
	DetailField,
	DetailLoadingState,
	DetailMetadata,
	DETAIL_HINT_CLASS,
	DETAIL_SECTION_CLASS,
	DETAIL_SECTION_TITLE_CLASS,
} from "./ArgoDetailShared";

function useArgoAppProjectDetails(
	project: ArgoAppProjectSummary,
	yamlViewMode: YamlViewMode = "kubectl",
	yamlEncoding: YamlEncoding = "yaml",
) {
	const client = useMemo(() => createTauriClient(), []);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigEnvVar);
	return useQuery({
		queryKey: [
			"argo-appproject-details",
			kubeconfigSourceKey(kubeconfigEnvVar),
			project.cluster,
			project.name,
			project.namespace,
			yamlViewMode,
			yamlEncoding,
		],
		queryFn: () =>
			getArgoAppProjectDetails(
				client,
				project.cluster,
				project.name,
			project.namespace ?? undefined,
			kubeconfigEnvVar,
			yamlViewMode,
			yamlEncoding,
		),
		enabled: !!project.cluster && !!project.name,
	});
}

export function ArgoAppProjectDetail({
	project,
}: {
	project: ArgoAppProjectSummary;
}) {
	const {
		data: details,
		isLoading,
		isError,
		error,
	} = useArgoAppProjectDetails(project);

	if (isLoading) return <DetailLoadingState label="Loading..." />;
	if (isError) {
		return <DetailErrorState title="Failed to load details" error={error} />;
	}
	if (!details) return null;

	return (
		<>
			<div className={DETAIL_HINT_CLASS}>
				Argo CD AppProject in {details.summary.namespace ?? "cluster-scoped"}
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Summary</div>
				<DetailField label="Description" value={details.summary.description} />
				<DetailField label="Status" value={details.summary.status} />
			</div>

			<DetailMetadata metadata={details.metadata as Record<string, unknown>} />
		</>
	);
}

export function ArgoAppProjectYaml({
	project,
	yamlViewMode,
	yamlEncoding,
}: {
	project: ArgoAppProjectSummary;
	yamlViewMode: YamlViewMode;
	yamlEncoding: YamlEncoding;
}) {
	const {
		data: details,
		isLoading,
		isError,
		error,
	} = useArgoAppProjectDetails(project, yamlViewMode, yamlEncoding);

	if (isLoading) return <DetailLoadingState label="Loading YAML..." />;
	if (isError) {
		return <DetailErrorState title="Failed to load YAML" error={error} />;
	}
	if (!details) return null;
	return <YamlCodeViewer value={details.yaml} minHeight="520px" />;
}
