import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
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
} from "@/lib/tauri";
import { YamlCodeViewer } from "@/components/YamlCodeViewer";
import {
	YamlEncodingControl,
	YamlViewModeControl,
} from "@/components/YamlModeControl";
import {
	type HelmReleaseSummary,
	type YamlEncoding,
	type YamlViewMode,
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
} from "@/features/argo/ArgoDetailShared";
import {
	helmStatusTone,
} from "./helpers";
import { HelmReconciliationPanel } from "./HelmReconciliationPanel";

type Tab = "details" | "reconciliation" | "yaml";

const PANEL_CLASS =
	"flex h-full min-w-0 flex-col overflow-hidden border-l bg-card";
const PANEL_HEADER_CLASS =
	"flex shrink-0 items-center justify-between border-b px-4 py-3";
const PANEL_TITLE_CLASS = "truncate whitespace-nowrap text-sm font-semibold";
const PANEL_TABS_CLASS = "flex shrink-0 border-b";
const PANEL_TAB_CLASS =
	"rounded-none border-b-2 border-transparent bg-transparent px-4 py-2 text-[13px] text-muted-foreground shadow-none transition-colors hover:text-foreground data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none";
const PANEL_BODY_CLASS = "flex-1 overflow-y-auto p-4";

function useHelmReleaseDetails(
	release: HelmReleaseSummary,
	yamlViewMode: YamlViewMode = "kubectl",
	yamlEncoding: YamlEncoding = "yaml",
) {
	const client = useMemo(() => createTauriClient(), []);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	return useQuery({
		queryKey: queryKeys.helmReleaseDetails(
			release.cluster,
			release.namespace,
			release.storageKind,
			release.storageName,
			kubeconfigEnvVar,
			yamlViewMode,
			yamlEncoding,
		),
		queryFn: () =>
			getHelmReleaseDetails(
				client,
				release,
				kubeconfigEnvVar,
				yamlViewMode,
				yamlEncoding,
			),
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

function HelmReleaseYaml({
	release,
	yamlViewMode,
	yamlEncoding,
}: {
	release: HelmReleaseSummary;
	yamlViewMode: YamlViewMode;
	yamlEncoding: YamlEncoding;
}) {
	const {
		data: details,
		isLoading,
		isError,
		error,
	} = useHelmReleaseDetails(release, yamlViewMode, yamlEncoding);

	if (isLoading) return <DetailLoadingState label="Loading YAML..." />;
	if (isError) {
		return <DetailErrorState title="Failed to load YAML" error={error} />;
	}
	if (!details) return null;
	return <YamlCodeViewer value={details.yaml} minHeight="520px" />;
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
	const yamlViewModeDefault = useSettingsState(
		(state) => state.yamlViewModeDefault,
	);
	const yamlEncodingDefault = useSettingsState(
		(state) => state.yamlEncodingDefault,
	);
	const [yamlViewMode, setYamlViewMode] = useState(yamlViewModeDefault);
	const [yamlEncoding, setYamlEncoding] = useState(yamlEncodingDefault);
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
						<TabsTrigger className={PANEL_TAB_CLASS} value="reconciliation">
							Reconciliation
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
					<TabsContent value="reconciliation" className="m-0">
						<HelmReconciliationPanel release={release} />
					</TabsContent>
					<TabsContent value="yaml" className="m-0">
						<div className="mb-3 flex flex-wrap justify-end gap-2">
							<YamlViewModeControl
								value={yamlViewMode}
								onChange={setYamlViewMode}
							/>
							<YamlEncodingControl
								value={yamlEncoding}
								onChange={setYamlEncoding}
							/>
						</div>
						<HelmReleaseYaml
							release={release}
							yamlViewMode={yamlViewMode}
							yamlEncoding={yamlEncoding}
						/>
					</TabsContent>
				</div>
			</Tabs>
		</div>
	);
}
