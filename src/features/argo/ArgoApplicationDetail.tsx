import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { StatusBadge } from "@/components/StatusBadge";
import { ExactTimestampText } from "@/components/TimestampText";
import { YamlCodeViewer } from "@/components/YamlCodeViewer";
import { kubeconfigSourceKey, useSettingsState } from "@/lib/settings";
import { createTauriClient, getArgoApplicationDetails } from "@/lib/tauri";
import type {
	ArgoApplicationSummary,
	YamlEncoding,
	YamlViewMode,
} from "@/lib/types";
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
} from "./ArgoDetailShared";
import { extractArgoStatusInsights } from "./status-insights";
import { healthStatusVariant } from "./status";

function useArgoApplicationDetails(
	app: ArgoApplicationSummary,
	yamlViewMode: YamlViewMode = "kubectl",
	yamlEncoding: YamlEncoding = "yaml",
) {
	const client = useMemo(() => createTauriClient(), []);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	return useQuery({
		queryKey: [
			"argo-app-details",
			kubeconfigSourceKey(kubeconfigEnvVar),
			app.cluster,
			app.name,
			app.namespace,
			yamlViewMode,
			yamlEncoding,
		],
		queryFn: () =>
			getArgoApplicationDetails(
				client,
				app.cluster,
				app.name,
			app.namespace ?? undefined,
			kubeconfigEnvVar,
			"kubectl",
			yamlEncoding,
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

	const insights = useMemo(
		() => extractArgoStatusInsights(details?.status),
		[details?.status],
	);

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
				{insights.healthMessage && (
					<DetailField label="Health Message" value={insights.healthMessage} />
				)}
				{insights.healthTransitionTime && (
					<div className="flex gap-3 border-b py-1.5">
						<span className="min-w-[120px] text-xs font-medium text-muted-foreground">
							Last Transition
						</span>
						<span className="min-w-0 flex-1 text-xs text-foreground">
							<ExactTimestampText value={insights.healthTransitionTime} />
						</span>
					</div>
				)}
			</div>

			{insights.unhealthyResources.length > 0 && (
				<div className={DETAIL_SECTION_CLASS}>
					<div className={DETAIL_SECTION_TITLE_CLASS}>Unhealthy Resources</div>
					<div className="flex flex-col gap-1.5">
						{insights.unhealthyResources.map((resource, index) => (
							<div
								key={`${resource.kind}:${resource.namespace}:${resource.name}:${index}`}
								className="rounded-md border bg-card px-3 py-2"
							>
								<div className="flex items-center justify-between gap-2">
									<span className="min-w-0 truncate text-xs font-semibold text-foreground">
										{resource.kind ?? "Resource"}/{resource.name ?? "unknown"}
									</span>
									{resource.health && (
										<StatusBadge tone={healthStatusVariant(resource.health)}>
											{resource.health}
										</StatusBadge>
									)}
								</div>
								{resource.message && (
									<div className="mt-1 text-xs leading-snug text-muted-foreground [overflow-wrap:anywhere]">
										{resource.message}
									</div>
								)}
							</div>
						))}
					</div>
				</div>
			)}

			{insights.conditions.length > 0 && (
				<div className={DETAIL_SECTION_CLASS}>
					<div className={DETAIL_SECTION_TITLE_CLASS}>Conditions</div>
					<div className="flex flex-col gap-1.5">
						{insights.conditions.map((condition, index) => (
							<div
								key={`${condition.type}:${index}`}
								className="rounded-md border bg-card px-3 py-2"
							>
								<div className="flex items-center justify-between gap-2">
									<span className="text-xs font-semibold text-foreground">
										{condition.type}
									</span>
									{condition.lastTransitionTime && (
										<ExactTimestampText
											value={condition.lastTransitionTime}
											className="text-[0.68rem] text-muted-foreground"
										/>
									)}
								</div>
								{condition.message && (
									<div className="mt-1 text-xs leading-snug text-muted-foreground [overflow-wrap:anywhere]">
										{condition.message}
									</div>
								)}
							</div>
						))}
					</div>
				</div>
			)}

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
				<details className={DETAIL_SECTION_CLASS}>
					<summary
						className={`${DETAIL_SECTION_TITLE_CLASS} cursor-pointer select-none`}
					>
						Raw Status (JSON)
					</summary>
					<pre className={JSON_BLOCK_CLASS}>
						{JSON.stringify(details.status, null, 2)}
					</pre>
				</details>
			)}

			<DetailMetadata metadata={details.metadata as Record<string, unknown>} />
		</>
	);
}

export function ArgoApplicationYaml({
	app,
	yamlViewMode,
	yamlEncoding,
}: {
	app: ArgoApplicationSummary;
	yamlViewMode: YamlViewMode;
	yamlEncoding: YamlEncoding;
}) {
	const {
		data: details,
		isLoading,
		isError,
		error,
	} = useArgoApplicationDetails(app, yamlViewMode, yamlEncoding);

	if (isLoading) return <DetailLoadingState label="Loading YAML..." />;
	if (isError) {
		return <DetailErrorState title="Failed to load YAML" error={error} />;
	}
	if (!details) return null;
	return <YamlCodeViewer value={details.yaml} minHeight="520px" />;
}
