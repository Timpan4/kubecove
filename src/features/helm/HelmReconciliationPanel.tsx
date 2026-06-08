import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import {
	Alert,
	AlertDescription,
	AlertTitle,
} from "@/components/ui/alert";
import { StatusBadge } from "@/components/StatusBadge";
import { Spinner } from "@/components/ui/spinner";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { queryKeys } from "@/lib/queryKeys";
import { useSettingsState } from "@/lib/settings";
import {
	createTauriClient,
	getHelmReleaseReconciliation,
} from "@/lib/tauri";
import type {
	HelmReconciliationResource,
	HelmReleaseSummary,
} from "@/lib/types";
import {
	DetailErrorState,
	DETAIL_HINT_CLASS,
	DETAIL_SECTION_CLASS,
	DETAIL_SECTION_TITLE_CLASS,
} from "@/features/argo/ArgoDetailShared";
import {
	helmReconciliationResourceLabel,
	helmReconciliationStatusLabel,
	helmReconciliationStatusTone,
	sortHelmReconciliationResources,
} from "./helpers";

function useHelmReconciliation(release: HelmReleaseSummary) {
	const client = useMemo(() => createTauriClient(), []);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigEnvVar);
	return useQuery({
		queryKey: queryKeys.helmReleaseReconciliation(
			release.cluster,
			release.namespace,
			release.storageKind,
			release.storageName,
			kubeconfigEnvVar,
		),
		queryFn: () =>
			getHelmReleaseReconciliation(client, release, kubeconfigEnvVar),
		enabled: !!release.cluster && !!release.namespace && !!release.storageName,
		staleTime: 30_000,
	});
}

function ReconciliationLoadingState() {
	return (
		<div className="flex min-h-32 items-center justify-center text-sm text-muted-foreground">
			<span className="inline-flex items-center gap-2">
				<Spinner className="size-4" />
				Loading reconciliation…
			</span>
		</div>
	);
}

function sourceLabel(resource: HelmReconciliationResource): string {
	if (resource.inManifest && resource.liveResource) return "Manifest + live";
	if (resource.inManifest) return "Manifest";
	if (resource.liveResource) return "Live label";
	return "-";
}

function totalsLabel({
	tracked,
	unlabeledLive,
	missing,
	labelOnly,
	unavailable,
}: {
	tracked: number;
	unlabeledLive: number;
	missing: number;
	labelOnly: number;
	unavailable: number;
}) {
	return `${tracked} tracked, ${unlabeledLive} unlabeled live, ${missing} missing, ${labelOnly} label-only, ${unavailable} unavailable`;
}

function ReconciliationWarnings({ warnings }: { warnings: string[] }) {
	if (warnings.length === 0) return null;
	return (
		<Alert className="mb-3">
			<AlertTitle>Reconciliation notes</AlertTitle>
			<AlertDescription>
				<ul className="flex list-disc flex-col gap-1 pl-4">
					{warnings.map((warning) => (
						<li key={warning}>{warning}</li>
					))}
				</ul>
			</AlertDescription>
		</Alert>
	);
}

function ReconciliationTable({
	resources,
}: {
	resources: HelmReconciliationResource[];
}) {
	const sorted = sortHelmReconciliationResources(resources);
	if (sorted.length === 0) {
		return (
			<div className={DETAIL_HINT_CLASS}>
				No manifest or explicit Helm-labeled live resources were found.
			</div>
		);
	}

	return (
		<div className="overflow-hidden rounded-md border">
			<Table className="w-full table-fixed text-xs">
				<TableHeader>
					<TableRow>
						<TableHead>Resource</TableHead>
						<TableHead>Namespace</TableHead>
						<TableHead>Status</TableHead>
						<TableHead>Source</TableHead>
						<TableHead>Message</TableHead>
					</TableRow>
				</TableHeader>
				<TableBody>
					{sorted.map((resource) => (
						<TableRow
							key={`${resource.apiVersion ?? ""}:${resource.kind ?? ""}:${resource.namespace ?? ""}:${resource.name ?? ""}:${resource.status}`}
						>
							<TableCell>{helmReconciliationResourceLabel(resource)}</TableCell>
							<TableCell>{resource.namespace ?? "-"}</TableCell>
							<TableCell>
								<StatusBadge
									tone={helmReconciliationStatusTone(resource.status)}
								>
									{helmReconciliationStatusLabel(resource.status)}
								</StatusBadge>
							</TableCell>
							<TableCell>{sourceLabel(resource)}</TableCell>
							<TableCell>{resource.statusMessage}</TableCell>
						</TableRow>
					))}
				</TableBody>
			</Table>
		</div>
	);
}

export function HelmReconciliationPanel({
	release,
}: {
	release: HelmReleaseSummary;
}) {
	const { data, isLoading, isError, error } = useHelmReconciliation(release);

	if (isLoading) return <ReconciliationLoadingState />;
	if (isError) {
		return <DetailErrorState title="Failed to reconcile release" error={error} />;
	}
	if (!data) return null;

	return (
		<div className={DETAIL_SECTION_CLASS}>
			<div className={DETAIL_SECTION_TITLE_CLASS}>Reconciliation</div>
			<div className={DETAIL_HINT_CLASS}>{totalsLabel(data.totals)}</div>
			<ReconciliationWarnings warnings={data.warnings} />
			<ReconciliationTable resources={data.resources} />
		</div>
	);
}
