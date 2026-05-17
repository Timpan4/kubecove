import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { createTauriClient, getArgoApplicationDetails } from "@/lib/tauri";
import type { ArgoApplicationSummary } from "@/lib/types";
import {
	DetailErrorState,
	DetailField,
	DetailLoadingState,
	DetailMetadata,
	DetailStatusField,
	DETAIL_HINT_CLASS,
	DETAIL_SECTION_CLASS,
	DETAIL_SECTION_TITLE_CLASS,
	healthStatusTone,
	JSON_BLOCK_CLASS,
	syncStatusTone,
	YAML_BLOCK_CLASS,
} from "./ArgoDetailShared";

function useArgoApplicationDetails(app: ArgoApplicationSummary) {
	const client = useMemo(() => createTauriClient(), []);
	return useQuery({
		queryKey: ["argo-app-details", app.cluster, app.name, app.namespace],
		queryFn: () =>
			getArgoApplicationDetails(
				client,
				app.cluster,
				app.name,
				app.namespace ?? undefined,
			),
		enabled: !!app.cluster && !!app.name,
	});
}

export function ArgoApplicationDetail({
	app,
}: {
	app: ArgoApplicationSummary;
}) {
	const {
		data: details,
		isLoading,
		isError,
		error,
	} = useArgoApplicationDetails(app);

	if (isLoading) return <DetailLoadingState label="Loading..." />;
	if (isError) {
		return <DetailErrorState title="Failed to load details" error={error} />;
	}
	if (!details) return null;

	return (
		<>
			<div className={DETAIL_HINT_CLASS}>
				Argo CD Application in {details.summary.namespace ?? "cluster-scoped"}
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Sync & Health</div>
				<DetailStatusField
					value={details.summary.syncStatus}
					label="Sync Status"
					tone={syncStatusTone(details.summary.syncStatus)}
				/>
				<DetailStatusField
					value={details.summary.healthStatus}
					label="Health Status"
					tone={healthStatusTone(details.summary.healthStatus)}
				/>
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
				<DetailField label="Project" value={details.summary.project} />
			</div>

			{details.status && Object.keys(details.status).length > 0 && (
				<div className={DETAIL_SECTION_CLASS}>
					<div className={DETAIL_SECTION_TITLE_CLASS}>Status Details</div>
					<pre className={JSON_BLOCK_CLASS}>
						{JSON.stringify(details.status, null, 2)}
					</pre>
				</div>
			)}

			<DetailMetadata metadata={details.metadata as Record<string, unknown>} />
		</>
	);
}

export function ArgoApplicationYaml({
	app,
}: {
	app: ArgoApplicationSummary;
}) {
	const {
		data: details,
		isLoading,
		isError,
		error,
	} = useArgoApplicationDetails(app);

	if (isLoading) return <DetailLoadingState label="Loading YAML..." />;
	if (isError) {
		return <DetailErrorState title="Failed to load YAML" error={error} />;
	}
	if (!details) return null;
	return <pre className={YAML_BLOCK_CLASS}>{details.yaml}</pre>;
}
