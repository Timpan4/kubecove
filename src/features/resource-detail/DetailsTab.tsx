import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Spinner } from "@/components/ui/spinner";
import { useMemo } from "react";
import type {
	ResourceDetailsFull,
	ResourceEventSummary,
	ResourceSummary,
} from "../../lib/types";
import { LOADING_STATE_CLASS } from "./constants";
import {
	buildIncidentSignals,
	getContainerStatusRows,
	getErrorMessage,
	type ConditionRow,
} from "./helpers";
import { IncidentSummary } from "./IncidentSummary";
import { IncidentTimeline } from "./IncidentTimeline";
import type { ParsedLogLine } from "./log-helpers";
import { ResourceDiagnostics } from "./ResourceDiagnostics";

interface DetailsTabProps {
	resource: ResourceSummary;
	details: ResourceDetailsFull | undefined;
	detailsLoading: boolean;
	detailsError: boolean;
	detailsErr: unknown;
	conditionRows: ConditionRow[];
	events: ResourceEventSummary[] | undefined;
	eventsLoading: boolean;
	eventsError: boolean;
	logLines?: ParsedLogLine[];
	onOpenHelmRelease?: (releaseName: string, namespace?: string | null) => void;
}

export function DetailsTab({
	resource,
	details,
	detailsLoading,
	detailsError,
	detailsErr,
	conditionRows,
	events,
	eventsLoading,
	eventsError,
	logLines,
	onOpenHelmRelease,
}: DetailsTabProps) {
	const currentResource = useMemo(
		() => (details?.summary ? { ...resource, ...details.summary } : resource),
		[details, resource],
	);
	const containerRows = useMemo(
		() => getContainerStatusRows(details?.status),
		[details?.status],
	);
	const podDetailsLoading =
		currentResource.kind === "Pod" && detailsLoading && !details && !detailsError;
	const signalContainers =
		currentResource.kind === "Pod" && (details || podDetailsLoading)
			? containerRows
			: undefined;
	const signals = useMemo(
		() =>
			buildIncidentSignals(
				currentResource,
				conditionRows,
				events ?? [],
				signalContainers,
			),
		[currentResource, conditionRows, events, signalContainers],
	);

	return (
		<>
			<IncidentSummary
				resource={currentResource}
				signals={signals}
				eventsLoading={eventsLoading}
				eventsError={eventsError}
			/>

			<IncidentTimeline
				resource={currentResource}
				conditions={conditionRows}
				events={events ?? []}
				containers={signalContainers}
				logLines={logLines}
			/>

			{detailsLoading && (
				<div className={LOADING_STATE_CLASS}>
					<Spinner className="mx-auto mb-2 size-4" />
					<span>Loading details...</span>
				</div>
			)}
			{detailsError && (
				<Alert variant="destructive">
					<AlertTitle>Failed to load details</AlertTitle>
					<AlertDescription>{getErrorMessage(detailsErr)}</AlertDescription>
				</Alert>
			)}

			<ResourceDiagnostics
				resource={currentResource}
				details={details}
				conditionRows={conditionRows}
				containerRows={containerRows}
				signals={signals}
				onOpenHelmRelease={onOpenHelmRelease}
			/>
		</>
	);
}
