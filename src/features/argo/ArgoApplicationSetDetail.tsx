import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { kubeconfigSourceKey, useSettingsState } from "@/lib/settings";
import { createTauriClient, getArgoApplicationSetDetails } from "@/lib/tauri";
import type { ArgoApplicationSetSummary } from "@/lib/types";
import {
	DetailErrorState,
	DetailField,
	DetailLoadingState,
	DetailMetadata,
	DETAIL_HINT_CLASS,
	DETAIL_SECTION_CLASS,
	DETAIL_SECTION_TITLE_CLASS,
	YAML_BLOCK_CLASS,
} from "./ArgoDetailShared";

function useArgoApplicationSetDetails(appset: ArgoApplicationSetSummary) {
	const client = useMemo(() => createTauriClient(), []);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigEnvVar);
	return useQuery({
		queryKey: [
			"argo-appset-details",
			kubeconfigSourceKey(kubeconfigEnvVar),
			appset.cluster,
			appset.name,
			appset.namespace,
		],
		queryFn: () =>
			getArgoApplicationSetDetails(
				client,
				appset.cluster,
				appset.name,
				appset.namespace ?? undefined,
				kubeconfigEnvVar,
			),
		enabled: !!appset.cluster && !!appset.name,
	});
}

export function ArgoApplicationSetDetail({
	appset,
}: {
	appset: ArgoApplicationSetSummary;
}) {
	const {
		data: details,
		isLoading,
		isError,
		error,
	} = useArgoApplicationSetDetails(appset);

	if (isLoading) return <DetailLoadingState label="Loading..." />;
	if (isError) {
		return <DetailErrorState title="Failed to load details" error={error} />;
	}
	if (!details) return null;

	return (
		<>
			<div className={DETAIL_HINT_CLASS}>
				Argo CD ApplicationSet in{" "}
				{details.summary.namespace ?? "cluster-scoped"}
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Summary</div>
				<DetailField label="Project" value={details.summary.project} />
				<DetailField label="Status" value={details.summary.status} />
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Destination</div>
				<DetailField
					label="Namespace"
					value={details.summary.destinationNamespace}
				/>
				<DetailField label="Server" value={details.summary.destinationServer} />
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Source</div>
				<DetailField label="Repository" value={details.summary.sourceRepo} />
				<DetailField label="Revision" value={details.summary.sourceRevision} />
			</div>

			<DetailMetadata metadata={details.metadata as Record<string, unknown>} />
		</>
	);
}

export function ArgoApplicationSetYaml({
	appset,
}: {
	appset: ArgoApplicationSetSummary;
}) {
	const {
		data: details,
		isLoading,
		isError,
		error,
	} = useArgoApplicationSetDetails(appset);

	if (isLoading) return <DetailLoadingState label="Loading YAML..." />;
	if (isError) {
		return <DetailErrorState title="Failed to load YAML" error={error} />;
	}
	if (!details) return null;
	return <pre className={YAML_BLOCK_CLASS}>{details.yaml}</pre>;
}
