import type { ParsedLogLine } from "./log-helpers";
import type { ResourceEventSummary, ResourceSummary } from "../../lib/types";
import type { ConditionRow, ContainerStatusRow } from "./helpers";
import { isCleanCompletedContainer } from "./helpers";

export type IncidentTimelineSource =
	| "status"
	| "condition"
	| "event"
	| "restart"
	| "container"
	| "log";

export type IncidentTimelineTone = "neutral" | "info" | "warning" | "error";

export interface IncidentTimelineItem {
	id: string;
	source: IncidentTimelineSource;
	tone: IncidentTimelineTone;
	title: string;
	detail?: string;
	timestamp?: string;
}

interface IncidentTimelineInput {
	resource: ResourceSummary;
	conditions: ConditionRow[];
	events: ResourceEventSummary[];
	containers?: ContainerStatusRow[];
	logLines?: ParsedLogLine[];
}

function isBadStatus(status: string | undefined): boolean {
	return [
		"failed",
		"error",
		"crashloopbackoff",
		"imagepullbackoff",
		"pending",
		"terminating",
		"unknown",
	].includes(status?.toLowerCase() ?? "");
}

function isSucceededResource(resource: ResourceSummary): boolean {
	return resource.status?.toLowerCase() === "succeeded";
}

function eventTimestamp(event: ResourceEventSummary): string | undefined {
	return event.lastSeenAt;
}

function timestampMs(value: string | undefined): number {
	if (!value) return Number.MAX_SAFE_INTEGER;
	const parsed = Date.parse(value);
	return Number.isNaN(parsed) ? Number.MAX_SAFE_INTEGER : parsed;
}

function pushUnique(
	items: IncidentTimelineItem[],
	seen: Set<string>,
	item: IncidentTimelineItem,
) {
	if (seen.has(item.id)) return;
	seen.add(item.id);
	items.push(item);
}

function restartTimestamp(container: ContainerStatusRow): string | undefined {
	return (
		container.lastFinishedAt ??
		container.lastStartedAt ??
		container.finishedAt ??
		container.startedAt
	);
}

function containerStateTone(container: ContainerStatusRow): IncidentTimelineTone {
	if (container.state === "waiting") return "warning";
	if (container.ready === false) return "error";
	if (container.exitCode !== undefined && container.exitCode !== 0) return "error";
	return "info";
}

function containerStateDetail(container: ContainerStatusRow): string {
	return [
		container.reason,
		container.message,
		container.exitCode !== undefined ? `exit ${container.exitCode}` : undefined,
	]
		.filter(Boolean)
		.join(" · ");
}

function shouldAddContainerState(container: ContainerStatusRow): boolean {
	if (!container.state || isCleanCompletedContainer(container)) return false;
	if (container.ready === false) return true;
	if (container.state === "waiting") return true;
	return container.state === "terminated" && container.exitCode !== 0;
}

function containerStateTimestamp(
	container: ContainerStatusRow,
): string | undefined {
	if (container.state === "terminated") {
		return container.finishedAt ?? container.startedAt;
	}
	return container.startedAt ?? container.finishedAt;
}

export function buildIncidentTimeline({
	resource,
	conditions,
	events,
	containers = [],
	logLines = [],
}: IncidentTimelineInput): IncidentTimelineItem[] {
	const items: IncidentTimelineItem[] = [];
	const seen = new Set<string>();

	if (resource.status && isBadStatus(resource.status)) {
		pushUnique(items, seen, {
			id: `status:${resource.status}`,
			source: "status",
			tone: ["failed", "error", "crashloopbackoff", "imagepullbackoff"].includes(
				resource.status.toLowerCase(),
			)
				? "error"
				: "warning",
			title: `Status ${resource.status}`,
		});
	}
	if (
		resource.ready?.toLowerCase() === "false" &&
		!isSucceededResource(resource)
	) {
		pushUnique(items, seen, {
			id: "status:ready:false",
			source: "status",
			tone: "error",
			title: "Ready False",
		});
	}

	for (const condition of conditions) {
		if (condition.status === "True") continue;
		pushUnique(items, seen, {
			id: `condition:${condition.type}:${condition.status}`,
			source: "condition",
			tone: condition.status === "False" ? "error" : "warning",
			title: `${condition.type} ${condition.status}`,
			detail: [condition.reason, condition.message].filter(Boolean).join(" · "),
			timestamp: condition.lastTransitionTime,
		});
	}

	for (const event of events.filter((item) => item.eventType === "Warning")) {
		pushUnique(items, seen, {
			id: `event:${event.reason}:${eventTimestamp(event) ?? event.message}`,
			source: "event",
			tone: "warning",
			title: `Warning ${event.reason}`,
			detail: `${event.message}${event.count > 1 ? ` · ${event.count} repeats` : ""}`,
			timestamp: eventTimestamp(event),
		});
	}

	for (const container of containers) {
		if (container.restartCount > 0) {
			pushUnique(items, seen, {
				id: `restart:${container.name}:${restartTimestamp(container) ?? "unknown"}`,
				source: "restart",
				tone: container.lastExitCode && container.lastExitCode !== 0 ? "error" : "warning",
				title: `${container.name} restarted`,
				detail: `${container.restartCount} ${
					container.restartCount === 1 ? "restart" : "restarts"
				}${container.lastReason ? ` · ${container.lastReason}` : ""}`,
				timestamp: restartTimestamp(container),
			});
		}
		if (shouldAddContainerState(container)) {
			const detail = containerStateDetail(container);
			pushUnique(items, seen, {
				id: `container:${container.name}:${container.state}`,
				source: "container",
				tone: containerStateTone(container),
				title: `${container.name} ${container.state}`,
				detail: detail || undefined,
				timestamp: containerStateTimestamp(container),
			});
		}
	}

	const latestLog = [...logLines]
		.filter((line) => line.timestamp)
		.sort((a, b) => timestampMs(b.timestamp) - timestampMs(a.timestamp))[0];
	if (latestLog?.timestamp) {
		pushUnique(items, seen, {
			id: `log:${latestLog.timestamp}:${latestLog.index}`,
			source: "log",
			tone: "neutral",
			title: "Latest log sample",
			detail: latestLog.message,
			timestamp: latestLog.timestamp,
		});
	}

	return items.sort((a, b) => {
		const timeDelta = timestampMs(a.timestamp) - timestampMs(b.timestamp);
		if (timeDelta !== 0) return timeDelta;
		return a.id.localeCompare(b.id);
	});
}
