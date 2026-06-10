import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
	YamlEncodingControl,
	YamlViewModeControl,
} from "@/components/YamlModeControl";
import { useSettingsState } from "@/lib/settings";
import { createTauriClient, getFluxResourceDetails } from "@/lib/tauri";
import type { FluxResourceSummary, YamlEncoding, YamlViewMode } from "@/lib/types";
import { queryKeys } from "@/lib/queryKeys";
import {
	DETAIL_HINT_CLASS,
	DETAIL_SECTION_CLASS,
	DETAIL_SECTION_TITLE_CLASS,
	DetailErrorState,
	DetailField,
	DetailLoadingState,
	DetailMetadata,
	DetailStatusField,
	JSON_BLOCK_CLASS,
	YAML_BLOCK_CLASS,
} from "@/features/argo/ArgoDetailShared";
import { fluxStatusTone } from "./flux-kinds";

type Tab = "details" | "yaml" | "status";

const PANEL_CLASS =
	"flex h-full min-w-0 flex-col overflow-hidden border-l bg-card";
const PANEL_HEADER_CLASS =
	"flex shrink-0 items-center justify-between border-b px-4 py-3";
const PANEL_TITLE_CLASS = "truncate whitespace-nowrap text-sm font-semibold";
const PANEL_TABS_CLASS = "flex shrink-0 border-b";
const PANEL_TAB_CLASS =
	"rounded-none border-b-2 border-transparent bg-transparent px-4 py-2 text-[13px] text-muted-foreground shadow-none transition-colors hover:text-foreground data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none";
const PANEL_BODY_CLASS = "flex-1 overflow-y-auto p-4";

export function FluxDetailPanel({
	resource,
	onClose,
}: {
	resource: FluxResourceSummary;
	onClose: () => void;
}) {
	const [activeTab, setActiveTab] = useState<Tab>("details");
	const yamlViewModeDefault = useSettingsState(
		(state) => state.yamlViewModeDefault,
	);
	const yamlEncodingDefault = useSettingsState(
		(state) => state.yamlEncodingDefault,
	);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	const [yamlViewMode, setYamlViewMode] = useState<YamlViewMode>(yamlViewModeDefault);
	const [yamlEncoding, setYamlEncoding] = useState<YamlEncoding>(yamlEncodingDefault);
	const client = useMemo(() => createTauriClient(), []);
	const { isPending, isError, error, data } = useQuery({
		queryKey: queryKeys.fluxResourceDetails(
			resource.cluster,
			resource.resourceKind,
			resource.name,
			resource.namespace,
			kubeconfigEnvVar,
			yamlViewMode,
			yamlEncoding,
		),
		queryFn: () =>
			getFluxResourceDetails(
				client,
				resource.cluster,
				resource.resourceKind,
				resource.name,
				resource.namespace,
				kubeconfigEnvVar,
				yamlViewMode,
				yamlEncoding,
			),
	});
	const title = `${resource.resourceKind.kind}/${resource.name}${
		resource.namespace ? ` (${resource.namespace})` : ""
	}`;

	return (
		<div className={PANEL_CLASS}>
			<div className={PANEL_HEADER_CLASS}>
				<span className={PANEL_TITLE_CLASS}>{title}</span>
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
						<TabsTrigger className={PANEL_TAB_CLASS} value="status">
							Status
						</TabsTrigger>
					</TabsList>
				</div>
				<div className={PANEL_BODY_CLASS}>
					{isPending ? (
						<DetailLoadingState label="Loading Flux resource..." />
					) : isError ? (
						<DetailErrorState title="Failed to load Flux resource" error={error} />
					) : !data ? (
						<DetailErrorState title="Failed to load Flux resource" error="No Flux resource data returned." />
					) : (
						<>
							<TabsContent value="details" className="m-0">
								<FluxDetails resource={data.summary} metadata={data.metadata} />
							</TabsContent>
							<TabsContent value="yaml" className="m-0">
								<div className="mb-3 flex flex-wrap justify-end gap-2">
									<YamlViewModeControl value={yamlViewMode} onChange={setYamlViewMode} />
									<YamlEncodingControl value={yamlEncoding} onChange={setYamlEncoding} />
								</div>
								<pre className={YAML_BLOCK_CLASS}>{data.yaml}</pre>
							</TabsContent>
							<TabsContent value="status" className="m-0">
								{data.status ? (
									<pre className={JSON_BLOCK_CLASS}>
										{JSON.stringify(data.status, null, 2)}
									</pre>
								) : (
									<p className={DETAIL_HINT_CLASS}>No status reported.</p>
								)}
							</TabsContent>
						</>
					)}
				</div>
			</Tabs>
		</div>
	);
}

function FluxDetails({
	resource,
	metadata,
}: {
	resource: FluxResourceSummary;
	metadata: Record<string, unknown>;
}) {
	return (
		<>
			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Flux</div>
				<DetailField label="Kind" value={resource.resourceKind.kind} />
				<DetailField label="API Version" value={resource.resourceKind.apiVersion} />
				<DetailField label="Namespace" value={resource.namespace} />
				<DetailStatusField
					label="Ready"
					value={resource.readyStatus}
					tone={fluxStatusTone(resource.readyStatus)}
				/>
				<DetailField
					label="Suspended"
					value={resource.suspended === undefined ? undefined : String(resource.suspended)}
				/>
				<DetailField label="Source" value={sourceLabel(resource)} />
				<DetailField label="Revision" value={resource.lastAppliedRevision} />
				<DetailField label="Interval" value={resource.interval} />
				<DetailField label="Message" value={resource.message} />
			</div>
			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Inventory</div>
				{resource.inventory.length > 0 ? (
					<pre className={JSON_BLOCK_CLASS}>
						{JSON.stringify(resource.inventory, null, 2)}
					</pre>
				) : (
					<p className={DETAIL_HINT_CLASS}>No inventory entries reported.</p>
				)}
			</div>
			<DetailMetadata metadata={metadata} />
		</>
	);
}

function sourceLabel(resource: FluxResourceSummary): string | undefined {
	if (!resource.sourceKind || !resource.sourceName) return undefined;
	return `${resource.sourceKind}/${resource.sourceName}`;
}
