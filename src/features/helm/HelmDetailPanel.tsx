import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { queryKeys } from "@/lib/queryKeys";
import { createTauriClient, getHelmReleaseDetails } from "@/lib/tauri";
import type { HelmReleaseSummary } from "@/lib/types";
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

type Tab = "details" | "yaml";

const PANEL_CLASS =
	"flex h-full min-w-0 flex-col overflow-hidden border-l bg-card";
const PANEL_HEADER_CLASS =
	"flex shrink-0 items-center justify-between border-b px-4 py-3";
const PANEL_TITLE_CLASS = "truncate whitespace-nowrap text-sm font-semibold";
const PANEL_TABS_CLASS = "flex shrink-0 border-b";
const PANEL_TAB_CLASS =
	"rounded-none border-b-2 border-transparent bg-transparent px-4 py-2 text-[13px] text-muted-foreground shadow-none transition-colors hover:text-foreground data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none";
const PANEL_BODY_CLASS = "flex-1 overflow-y-auto p-4";

function statusTone(status: string | undefined) {
	if (status === "deployed" || status === "superseded") return "success";
	if (status === "failed" || status === "uninstalled") return "error";
	if (status === "pending-install" || status === "pending-upgrade") return "warning";
	return "neutral";
}

function useHelmReleaseDetails(release: HelmReleaseSummary) {
	const client = useMemo(() => createTauriClient(), []);
	return useQuery({
		queryKey: queryKeys.helmReleaseDetails(
			release.cluster,
			release.namespace,
			release.storageKind,
			release.storageName,
		),
		queryFn: () => getHelmReleaseDetails(client, release),
		enabled: !!release.cluster && !!release.namespace && !!release.storageName,
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
					tone={statusTone(details.summary.status)}
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
						<TabsTrigger className={PANEL_TAB_CLASS} value="yaml">
							YAML
						</TabsTrigger>
					</TabsList>
				</div>
				<div className={PANEL_BODY_CLASS}>
					<TabsContent value="details" className="m-0">
						<HelmReleaseDetail release={release} />
					</TabsContent>
					<TabsContent value="yaml" className="m-0">
						<HelmReleaseYaml release={release} />
					</TabsContent>
				</div>
			</Tabs>
		</div>
	);
}
