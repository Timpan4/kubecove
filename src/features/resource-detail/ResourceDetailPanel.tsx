import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { X } from "lucide-react";
import { memo, useEffect, useMemo, useRef, useState } from "react";
import { createTauriClient } from "../../lib/tauri";
import type { ResourceSummary } from "../../lib/types";
import { diagnosticLog } from "../../lib/diagnostics";
import {
	PANEL_BODY_CLASS,
	PANEL_CLASS,
	PANEL_HEADER_CLASS,
	PANEL_TAB_CLASS,
	PANEL_TABS_CLASS,
	PANEL_TITLE_CLASS,
	type Tab,
} from "./constants";
import { DetailsTab } from "./DetailsTab";
import { EventsTab } from "./EventsTab";
import {
	dynamicResourceKindFromSummary,
	getConditionRows,
	getContainerStatusRows,
} from "./helpers";
import { LogsTab } from "./LogsTab";
import type { ParsedLogLine } from "./log-helpers";
import { useResourceDetails } from "./useResourceDetails";
import { YamlTab } from "./YamlTab";

interface ResourceDetailPanelProps {
	resource: ResourceSummary;
	onClose: () => void;
	onOpenHelmRelease?: (releaseName: string, namespace?: string | null) => void;
}

export const ResourceDetailPanel = memo(function ResourceDetailPanel({
	resource,
	onClose,
	onOpenHelmRelease,
}: ResourceDetailPanelProps) {
	const [activeTab, setActiveTab] = useState<Tab>("details");
	const [selectedContainer, setSelectedContainer] = useState("");
	const [timelineLogLine, setTimelineLogLine] = useState<
		ParsedLogLine | undefined
	>();
	const client = useMemo(() => createTauriClient(), []);
	const dynamicResourceKind = useMemo(
		() => dynamicResourceKindFromSummary(resource),
		[resource],
	);
	const resourceKey = `${resource.cluster}:${resource.apiVersion ?? ""}:${resource.kind}:${resource.namespace ?? ""}:${resource.name}`;
	const renderCountRef = useRef(0);
	renderCountRef.current += 1;

	useEffect(() => {
		diagnosticLog("detail.resource.changed", {
			key: resourceKey,
			render: renderCountRef.current,
		});
		setActiveTab("details");
		setTimelineLogLine(undefined);
	}, [resourceKey]);

	useEffect(() => {
		diagnosticLog("detail.mount", { key: resourceKey });
		return () => {
			diagnosticLog("detail.unmount", { key: resourceKey });
		};
	}, [resourceKey]);

	const {
		detailsEnabled,
		yamlEnabled,
		eventsEnabled,
		detailsQuery,
		yamlQuery,
		eventsQuery,
	} = useResourceDetails({
		resource,
		activeTab,
		resourceKey,
		client,
		dynamicResourceKind,
	});
	const { data: details } = detailsQuery;
	const { data: yaml } = yamlQuery;
	const { data: events } = eventsQuery;

	useEffect(() => {
		diagnosticLog("detail.render", {
			key: resourceKey,
			render: renderCountRef.current,
			tab: activeTab,
			detailsEnabled,
			detailsLoading: detailsQuery.isLoading,
			yamlEnabled,
			yamlLoading: yamlQuery.isLoading,
			eventsEnabled,
			eventsLoading: eventsQuery.isLoading,
			hasDetails: Boolean(details),
			hasYaml: Boolean(yaml),
			hasEvents: Boolean(events),
		});
	});

	const conditionRows = useMemo(
		() => getConditionRows(details?.status),
		[details?.status],
	);
	const containerRows = useMemo(
		() => getContainerStatusRows(details?.status),
		[details?.status],
	);
	const containerSignature = containerRows
		.map((container) => `${container.type ?? "container"}:${container.name}`)
		.join("|");

	useEffect(() => {
		if (resource.kind !== "Pod") {
			setSelectedContainer("");
			return;
		}
		if (containerRows.length === 0) return;
		if (
			selectedContainer &&
			containerRows.some((container) => container.name === selectedContainer)
		) {
			return;
		}
		const regularContainer =
			containerRows.find((container) => container.type === "container") ??
			containerRows[0];
		setSelectedContainer(regularContainer?.name ?? "");
	}, [containerRows, containerSignature, resource.kind, selectedContainer]);

	return (
		<div className={PANEL_CLASS}>
			<div className={PANEL_HEADER_CLASS}>
				<span className={PANEL_TITLE_CLASS}>{resource.name}</span>
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
				onValueChange={(value) => {
					const tab = value as Tab;
					diagnosticLog("detail.tab.click", {
						key: resourceKey,
						tab,
					});
					setActiveTab(tab);
				}}
				className="min-h-0 flex-1 gap-0"
			>
				<div className={PANEL_TABS_CLASS}>
					<TabsList className="h-auto rounded-none bg-transparent p-0">
						<TabsTrigger className={PANEL_TAB_CLASS} value="details">
							Details
						</TabsTrigger>
						<TabsTrigger className={PANEL_TAB_CLASS} value="events">
							Events
						</TabsTrigger>
						{resource.kind === "Pod" && (
							<TabsTrigger className={PANEL_TAB_CLASS} value="logs">
								Logs
							</TabsTrigger>
						)}
						<TabsTrigger className={PANEL_TAB_CLASS} value="yaml">
							YAML
						</TabsTrigger>
					</TabsList>
				</div>
				<div className={PANEL_BODY_CLASS}>
					<TabsContent value="details" className="m-0">
						<DetailsTab
							resource={resource}
							details={details}
							detailsLoading={detailsQuery.isLoading}
							detailsError={detailsQuery.isError}
							detailsErr={detailsQuery.error}
							conditionRows={conditionRows}
							events={events}
							eventsLoading={eventsQuery.isLoading}
							eventsError={eventsQuery.isError}
							logLines={timelineLogLine ? [timelineLogLine] : undefined}
							onOpenHelmRelease={onOpenHelmRelease}
						/>
					</TabsContent>
					<TabsContent value="events" className="m-0">
						<EventsTab
							events={events}
							eventsLoading={eventsQuery.isLoading}
							eventsError={eventsQuery.isError}
							eventsErr={eventsQuery.error}
						/>
					</TabsContent>
					{resource.kind === "Pod" && (
						<TabsContent value="logs" className="m-0 h-full min-h-0">
							<LogsTab
								client={client}
								resource={resource}
								containers={containerRows}
								selectedContainer={selectedContainer}
								onSelectedContainerChange={setSelectedContainer}
								onLatestLogLineChange={setTimelineLogLine}
								active={activeTab === "logs"}
							/>
						</TabsContent>
					)}
					<TabsContent value="yaml" className="m-0">
						<YamlTab
							yaml={yaml}
							yamlLoading={yamlQuery.isLoading}
							yamlError={yamlQuery.isError}
							yamlErr={yamlQuery.error}
						/>
					</TabsContent>
				</div>
			</Tabs>
		</div>
	);
});
