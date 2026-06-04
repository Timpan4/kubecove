import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { queryKeys } from "@/lib/queryKeys";
import { useSettingsState } from "@/lib/settings";
import {
	createTauriClient,
	getHelmReleaseDetails,
	listResources,
	type TauriClient,
} from "@/lib/tauri";
import {
	SUPPORTED_KINDS,
	type HelmReleaseSummary,
	type ResourceSummary,
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
	JSON_BLOCK_CLASS,
	YAML_BLOCK_CLASS,
} from "@/features/argo/ArgoDetailShared";
import {
	helmReleaseResourceLabel,
	helmStatusTone,
	resourcesOwnedByHelmRelease,
} from "./helpers";

type Tab = "details" | "resources" | "yaml";

const PANEL_CLASS =
	"flex h-full min-w-0 flex-col overflow-hidden border-l bg-card";
const PANEL_HEADER_CLASS =
	"flex shrink-0 items-center justify-between border-b px-4 py-3";
const PANEL_TITLE_CLASS = "truncate whitespace-nowrap text-sm font-semibold";
const PANEL_TABS_CLASS = "flex shrink-0 border-b";
const PANEL_TAB_CLASS =
	"rounded-none border-b-2 border-transparent bg-transparent px-4 py-2 text-[13px] text-muted-foreground shadow-none transition-colors hover:text-foreground data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none";
const PANEL_BODY_CLASS = "flex-1 overflow-y-auto p-4";

function useHelmReleaseDetails(release: HelmReleaseSummary) {
	const client = useMemo(() => createTauriClient(), []);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigEnvVar);
	return useQuery({
		queryKey: queryKeys.helmReleaseDetails(
			release.cluster,
			release.namespace,
			release.storageKind,
			release.storageName,
			kubeconfigEnvVar,
		),
		queryFn: () => getHelmReleaseDetails(client, release, kubeconfigEnvVar),
		enabled: !!release.cluster && !!release.namespace && !!release.storageName,
	});
}

async function listHelmOwnedResources(
	client: TauriClient,
	release: HelmReleaseSummary,
	kubeconfigEnvVar?: string,
): Promise<ResourceSummary[]> {
	const results = await Promise.allSettled(
		SUPPORTED_KINDS.map((kind) =>
			listResources(client, release.cluster, kind, release.namespace, kubeconfigEnvVar),
		),
	);
	const rejected = results.filter(
		(result): result is PromiseRejectedResult => result.status === "rejected",
	);
	if (rejected.length > 0) {
		throw new Error("Failed to list Helm-owned resources");
	}

	const fulfilled = results.filter(
		(result): result is PromiseFulfilledResult<ResourceSummary[]> =>
			result.status === "fulfilled",
	);
	const resources = fulfilled.flatMap((result) => result.value);
	return resourcesOwnedByHelmRelease(resources, release);
}

function useHelmOwnedResources(release: HelmReleaseSummary) {
	const client = useMemo(() => createTauriClient(), []);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigEnvVar);
	return useQuery({
		queryKey: queryKeys.helmReleaseResources(
			release.cluster,
			release.namespace,
			release.name,
			kubeconfigEnvVar,
		),
		queryFn: () => listHelmOwnedResources(client, release, kubeconfigEnvVar),
		enabled: !!release.cluster && !!release.namespace && !!release.name,
		staleTime: 30_000,
	});
}

function HelmReleaseDetail({ release }: { release: HelmReleaseSummary }) {
	const {
		data: details,
		isLoading,
		isError,
		error,
	} = useHelmReleaseDetails(release);

	if (isLoading) return <DetailLoadingState label="Loading..." />;
	if (isError) {
		return <DetailErrorState title="Failed to load details" error={error} />;
	}
	if (!details) return null;

	return (
		<>
			<div className={DETAIL_HINT_CLASS}>
				Helm release in {details.summary.namespace}
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Release</div>
				<DetailField label="Name" value={details.summary.name} />
				<DetailField label="Namespace" value={details.summary.namespace} />
				<DetailField
					label="Revision"
					value={details.summary.revision?.toString()}
				/>
				<DetailStatusField
					value={details.summary.status}
					label="Status"
					tone={helmStatusTone(details.summary.status)}
				/>
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Chart</div>
				<DetailField label="Chart" value={details.summary.chart} />
				<DetailField label="App Version" value={details.summary.appVersion} />
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Storage</div>
				<DetailField label="Kind" value={details.summary.storageKind} />
				<DetailField label="Name" value={details.summary.storageName} />
				<DetailField label="Updated" value={details.summary.updatedAt} />
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Values</div>
				<DetailField
					label="Top-level keys"
					value={
						details.valuesSummary.topLevelKeys.length > 0
							? details.valuesSummary.topLevelKeys.join(", ")
							: details.valuesSummary.hasValues
								? `${details.valuesSummary.valueCount} non-object value set`
								: "No decoded values"
					}
				/>
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Manifest</div>
				<DetailField
					label="Resources"
					value={
						details.manifestSummary.resourceCount > 0
							? `${details.manifestSummary.resourceCount}${details.manifestSummary.truncated ? "+" : ""}`
							: "No decoded manifest resources"
					}
				/>
				{details.manifestSummary.resources.length > 0 && (
					<div className="mt-2 overflow-hidden rounded-md border">
						<Table className="w-full table-fixed text-xs">
							<TableHeader>
								<TableRow>
									<TableHead>Kind</TableHead>
									<TableHead>Name</TableHead>
									<TableHead>Namespace</TableHead>
									<TableHead>API</TableHead>
								</TableRow>
							</TableHeader>
							<TableBody>
								{details.manifestSummary.resources.slice(0, 8).map((resource) => (
									<TableRow
										key={`${resource.apiVersion ?? ""}:${resource.kind ?? ""}:${resource.namespace ?? ""}:${resource.name ?? ""}`}
									>
										<TableCell>{resource.kind ?? "-"}</TableCell>
										<TableCell>{resource.name ?? "-"}</TableCell>
										<TableCell>{resource.namespace ?? "-"}</TableCell>
										<TableCell>{resource.apiVersion ?? "-"}</TableCell>
									</TableRow>
								))}
							</TableBody>
						</Table>
					</div>
				)}
			</div>

			{details.release && Object.keys(details.release).length > 0 && (
				<div className={DETAIL_SECTION_CLASS}>
					<div className={DETAIL_SECTION_TITLE_CLASS}>Decoded Metadata</div>
					<pre className={JSON_BLOCK_CLASS}>
						{JSON.stringify(details.release, null, 2)}
					</pre>
				</div>
			)}

			<DetailMetadata metadata={details.metadata} />
		</>
	);
}

function HelmReleaseResources({ release }: { release: HelmReleaseSummary }) {
	const {
		data: resources,
		isLoading,
		isError,
		error,
	} = useHelmOwnedResources(release);

	if (isLoading) {
		return (
			<div className="flex min-h-32 items-center justify-center text-sm text-muted-foreground">
				<span className="inline-flex items-center gap-2">
					<Spinner className="size-4" />
					Loading owned resources...
				</span>
			</div>
		);
	}
	if (isError) {
		return <DetailErrorState title="Failed to load resources" error={error} />;
	}
	if (!resources || resources.length === 0) {
		return (
			<div className={DETAIL_HINT_CLASS}>
				No resources with matching Helm ownership labels were found.
			</div>
		);
	}

	return (
		<div className={DETAIL_SECTION_CLASS}>
			<div className={DETAIL_SECTION_TITLE_CLASS}>
				Owned resources ({resources.length})
			</div>
			<div className="overflow-hidden rounded-md border">
				<Table className="w-full table-fixed text-xs">
					<TableHeader>
						<TableRow>
							<TableHead>Resource</TableHead>
							<TableHead>Status</TableHead>
							<TableHead>Ready</TableHead>
							<TableHead>Age</TableHead>
						</TableRow>
					</TableHeader>
					<TableBody>
						{resources.map((resource) => (
							<TableRow
								key={`${resource.apiVersion ?? ""}:${resource.kind}:${resource.namespace ?? ""}:${resource.name}`}
							>
								<TableCell>{helmReleaseResourceLabel(resource)}</TableCell>
								<TableCell>{resource.status ?? "-"}</TableCell>
								<TableCell>{resource.ready ?? "-"}</TableCell>
								<TableCell>{resource.age}</TableCell>
							</TableRow>
						))}
					</TableBody>
				</Table>
			</div>
		</div>
	);
}

function HelmReleaseYaml({ release }: { release: HelmReleaseSummary }) {
	const {
		data: details,
		isLoading,
		isError,
		error,
	} = useHelmReleaseDetails(release);

	if (isLoading) return <DetailLoadingState label="Loading YAML..." />;
	if (isError) {
		return <DetailErrorState title="Failed to load YAML" error={error} />;
	}
	if (!details) return null;
	return <pre className={YAML_BLOCK_CLASS}>{details.yaml}</pre>;
}

export function HelmDetailPanel({
	release,
	onClose,
	onOpenResources,
}: {
	release: HelmReleaseSummary;
	onClose: () => void;
	onOpenResources: (release: HelmReleaseSummary) => void;
}) {
	const [activeTab, setActiveTab] = useState<Tab>("details");
	const title = `${release.name} (${release.namespace})`;

	return (
		<div className={PANEL_CLASS}>
			<div className={PANEL_HEADER_CLASS}>
				<span className={PANEL_TITLE_CLASS}>{title}</span>
				<div className="flex items-center gap-1">
					<Button
						type="button"
						variant="outline"
						size="sm"
						onClick={() => onOpenResources(release)}
					>
						View resources
					</Button>
					<Button
						type="button"
						variant="ghost"
						size="icon"
						className="size-7 text-muted-foreground"
						onClick={onClose}
						aria-label="Close panel"
					>
						<X className="size-4" />
					</Button>
				</div>
			</div>
			<Tabs
				value={activeTab}
				onValueChange={(value) => setActiveTab(value as Tab)}
				className="min-h-0 flex-1 gap-0"
			>
				<div className={PANEL_TABS_CLASS}>
					<TabsList className="h-auto rounded-none bg-transparent p-0">
						<TabsTrigger className={PANEL_TAB_CLASS} value="details">
							Details
						</TabsTrigger>
						<TabsTrigger className={PANEL_TAB_CLASS} value="resources">
							Resources
						</TabsTrigger>
						<TabsTrigger className={PANEL_TAB_CLASS} value="yaml">
							YAML
						</TabsTrigger>
					</TabsList>
				</div>
				<div className={PANEL_BODY_CLASS}>
					<TabsContent value="details" className="m-0">
						<HelmReleaseDetail release={release} />
					</TabsContent>
					<TabsContent value="resources" className="m-0">
						<HelmReleaseResources release={release} />
					</TabsContent>
					<TabsContent value="yaml" className="m-0">
						<HelmReleaseYaml release={release} />
					</TabsContent>
				</div>
			</Tabs>
		</div>
	);
}
