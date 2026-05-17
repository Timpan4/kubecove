import type {
	DiscoveredResourceKind,
	ResourceEventSummary,
	ResourceSummary,
} from "../../lib/types";
import type { ChipVariant } from "./constants";

export interface ConditionRow {
	type: string;
	status: string;
	reason?: string;
	message?: string;
	lastTransitionTime?: string;
}

export interface ContainerStatusRow {
	name: string;
	ready?: boolean;
	restartCount: number;
	state?: string;
	reason?: string;
	message?: string;
	startedAt?: string;
	finishedAt?: string;
	exitCode?: number;
	lastState?: string;
	lastReason?: string;
	lastMessage?: string;
	lastStartedAt?: string;
	lastFinishedAt?: string;
	lastExitCode?: number;
}

export function shouldFetchResourceDetails(
	resource: Pick<ResourceSummary, "cluster" | "kind" | "name">,
): boolean {
	return (
		Boolean(resource.cluster) &&
		Boolean(resource.kind) &&
		Boolean(resource.name)
	);
}

export function shouldFetchResourceEvents(
	resource: Pick<ResourceSummary, "cluster" | "kind" | "name">,
): boolean {
	return shouldFetchResourceDetails(resource);
}

export function dynamicResourceKindFromSummary(
	resource: ResourceSummary,
): DiscoveredResourceKind | null {
	if (
		!resource.dynamic ||
		!resource.apiVersion ||
		resource.version === undefined ||
		!resource.kind ||
		!resource.plural ||
		resource.namespaced === undefined
	) {
		return null;
	}

	return {
		group: resource.group ?? "",
		version: resource.version,
		apiVersion: resource.apiVersion,
		kind: resource.kind,
		plural: resource.plural,
		namespaced: resource.namespaced,
	};
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function getConditionRows(
	status: Record<string, unknown> | undefined,
): ConditionRow[] {
	if (!status || !Array.isArray(status.conditions)) return [];
	return status.conditions.filter(isRecord).map((condition) => ({
		type: String(condition.type ?? "Condition"),
		status: String(condition.status ?? "Unknown"),
		reason:
			typeof condition.reason === "string" ? condition.reason : undefined,
		message:
			typeof condition.message === "string" ? condition.message : undefined,
		lastTransitionTime:
			typeof condition.lastTransitionTime === "string"
				? condition.lastTransitionTime
				: undefined,
	}));
}

function stringField(record: Record<string, unknown>, key: string): string | undefined {
	const value = record[key];
	return typeof value === "string" ? value : undefined;
}

function numberField(record: Record<string, unknown>, key: string): number | undefined {
	const value = record[key];
	return typeof value === "number" ? value : undefined;
}

interface ParsedContainerState {
	state?: string;
	reason?: string;
	message?: string;
	startedAt?: string;
	finishedAt?: string;
	exitCode?: number;
}

function parseStateFields(state: unknown): ParsedContainerState {
	if (!isRecord(state)) return {};
	const stateEntry = Object.entries(state).find(([, value]) => isRecord(value));
	if (!stateEntry) return {};
	const [stateName, stateValue] = stateEntry;
	if (!isRecord(stateValue)) return {};

	return {
		state: stateName,
		reason: stringField(stateValue, "reason"),
		message: stringField(stateValue, "message"),
		startedAt: stringField(stateValue, "startedAt"),
		finishedAt: stringField(stateValue, "finishedAt"),
		exitCode: numberField(stateValue, "exitCode"),
	};
}

function getStatusList(status: Record<string, unknown>, key: string): Record<string, unknown>[] {
	const value = status[key];
	return Array.isArray(value) ? value.filter(isRecord) : [];
}

export function getContainerStatusRows(
	status: Record<string, unknown> | undefined,
): ContainerStatusRow[] {
	if (!status) return [];
	return [
		...getStatusList(status, "initContainerStatuses"),
		...getStatusList(status, "containerStatuses"),
		...getStatusList(status, "ephemeralContainerStatuses"),
	].map((container) => {
		const currentState = parseStateFields(container.state);
		const lastState = parseStateFields(container.lastState);
		return {
			name: String(container.name ?? "container"),
			ready: typeof container.ready === "boolean" ? container.ready : undefined,
			restartCount:
				typeof container.restartCount === "number" ? container.restartCount : 0,
			state: currentState.state,
			reason: currentState.reason,
			message: currentState.message,
			startedAt: currentState.startedAt,
			finishedAt: currentState.finishedAt,
			exitCode: currentState.exitCode,
			lastState: lastState.state,
			lastReason: lastState.reason,
			lastMessage: lastState.message,
			lastStartedAt: lastState.startedAt,
			lastFinishedAt: lastState.finishedAt,
			lastExitCode: lastState.exitCode,
		};
	});
}

export interface IncidentSignal {
	id: string;
	label: string;
	value: string;
	tone: ChipVariant;
	source: "status" | "condition" | "event";
}

interface IncidentSignalOptions {
	now?: Date;
	staleRestartMs?: number;
}

export function incidentSignalCardClassName(tone: ChipVariant): string {
	const base = "rounded-md border border-l-4 p-3";
	switch (tone) {
		case "error":
			return `${base} border-red-500/25 border-l-red-500 bg-red-500/5`;
		case "warning":
			return `${base} border-amber-500/25 border-l-amber-500 bg-amber-500/5`;
		case "info":
			return `${base} border-sky-500/25 border-l-sky-500 bg-sky-500/5`;
		case "success":
			return `${base} border-emerald-500/25 border-l-emerald-500 bg-emerald-500/5`;
		case "neutral":
			return `${base} border-border border-l-muted bg-card`;
	}
}

function eventWarningSummary(events: ResourceEventSummary[]): string | null {
	const warnings = events.filter((event) => event.eventType === "Warning");
	if (warnings.length === 0) return null;
	const reasons = new Set(warnings.map((event) => event.reason));
	const repeats = warnings.reduce((total, event) => total + event.count, 0);
	const reasonLabel = reasons.size === 1 ? "warning reason" : "warning reasons";
	const repeatLabel = repeats === 1 ? "repeat" : "repeats";
	return `${reasons.size} ${reasonLabel} · ${repeats} ${repeatLabel}`;
}

function isRecentTimestamp(
	timestamp: string | undefined,
	now: Date,
	staleRestartMs: number,
): boolean {
	if (!timestamp) return true;
	const value = Date.parse(timestamp);
	if (Number.isNaN(value)) return true;
	return now.getTime() - value <= staleRestartMs;
}

function isSucceededResource(resource: ResourceSummary): boolean {
	return resource.status?.toLowerCase() === "succeeded";
}

export function isCleanCompletedContainer(
	container: ContainerStatusRow,
): boolean {
	return (
		container.state === "terminated" &&
		container.reason === "Completed" &&
		(container.exitCode === undefined || container.exitCode === 0)
	);
}

function isActionableContainerRestart(
	container: ContainerStatusRow,
	now: Date,
	staleRestartMs: number,
	succeededResource: boolean,
): boolean {
	if (container.restartCount <= 0) return false;
	if (succeededResource && isCleanCompletedContainer(container)) return false;
	if (container.ready === false) return true;
	if (container.state === "waiting") return true;
	if (container.state === "terminated" && container.exitCode !== 0) return true;
	if (container.lastExitCode !== undefined && container.lastExitCode !== 0) return true;
	if (container.lastReason && container.lastReason !== "Completed") return true;
	return isRecentTimestamp(container.lastFinishedAt, now, staleRestartMs);
}

function restartSignalValue(containers: ContainerStatusRow[]): string {
	const total = containers.reduce((sum, container) => sum + container.restartCount, 0);
	const label = containers
		.map((container) => {
			const details = [
				`${container.name} restarted ${container.restartCount} ${
					container.restartCount === 1 ? "time" : "times"
				}`,
				container.lastReason,
				container.lastExitCode !== undefined ? `exit ${container.lastExitCode}` : undefined,
				container.lastFinishedAt ? `finished ${container.lastFinishedAt}` : undefined,
			].filter(Boolean);
			return details.join(" · ");
		})
		.join("; ");
	return label || String(total);
}

function restartSignalTone(
	containers: ContainerStatusRow[] | undefined,
	restarts: number,
): ChipVariant {
	const actionableRestarts =
		containers?.reduce((sum, container) => sum + container.restartCount, 0) ??
		restarts;
	return actionableRestarts > 5 ? "error" : "warning";
}

function isCompletedReadinessCondition(condition: ConditionRow): boolean {
	if (
		condition.reason === "PodCompleted" &&
		["Ready", "ContainersReady"].includes(condition.type)
	) {
		return true;
	}
	return condition.type === "PodReadyToStartContainers";
}

function isDisruptionTargetCondition(condition: ConditionRow): boolean {
	return condition.type === "DisruptionTarget" && condition.status === "True";
}

function conditionSignalValue(condition: ConditionRow): string {
	return [
		`${condition.type}=${condition.status}`,
		condition.reason,
		condition.lastTransitionTime
			? `since ${condition.lastTransitionTime}`
			: undefined,
	].filter(Boolean).join(" · ");
}

export function buildIncidentSignals(
	resource: ResourceSummary,
	conditions: ConditionRow[],
	events: ResourceEventSummary[],
	containers?: ContainerStatusRow[],
	options: IncidentSignalOptions = {},
): IncidentSignal[] {
	const signals: IncidentSignal[] = [];
	const status = resource.status?.toLowerCase();
	const ready = resource.ready?.toLowerCase();
	const restarts = resource.restarts ?? 0;
	const now = options.now ?? new Date();
	const staleRestartMs = options.staleRestartMs ?? 60 * 60 * 1000;
	const succeededResource = isSucceededResource(resource);

	if (
		resource.status &&
		[
			"failed",
			"error",
			"crashloopbackoff",
			"imagepullbackoff",
			"pending",
			"terminating",
			"unknown",
		].includes(status ?? "")
	) {
		const isErrorStatus =
			status === "failed" ||
			status === "error" ||
			status === "crashloopbackoff" ||
			status === "imagepullbackoff";
		signals.push({
			id: "status",
			label: "Status",
			value: resource.status,
			tone: isErrorStatus ? "error" : "warning",
			source: "status",
		});
	}

	if (resource.ready && ready === "false" && !succeededResource) {
		signals.push({
			id: "ready",
			label: "Ready",
			value: resource.ready,
			tone: "error",
			source: "status",
		});
	}

	const restartedContainers = containers?.filter(
		(container) => container.restartCount > 0,
	);
	const actionableRestartContainers = restartedContainers?.filter((container) =>
		isActionableContainerRestart(
			container,
			now,
			staleRestartMs,
			succeededResource,
		),
	);
	const shouldShowRestartSignal = containers
		? Boolean(actionableRestartContainers?.length)
		: restarts > 0;

	if (shouldShowRestartSignal) {
		const restartValue =
			actionableRestartContainers && actionableRestartContainers.length > 0
				? restartSignalValue(actionableRestartContainers)
				: String(restarts);
		signals.push({
			id: "restarts",
			label: "Restarts",
			value: restartValue,
			tone: restartSignalTone(actionableRestartContainers, restarts),
			source: "status",
		});
	}

	for (const condition of conditions) {
		if (isDisruptionTargetCondition(condition)) {
			signals.push({
				id: `condition:${condition.type}`,
				label: "Condition",
				value: conditionSignalValue(condition),
				tone: "info",
				source: "condition",
			});
			continue;
		}
		if (condition.status === "True") continue;
		if (succeededResource && isCompletedReadinessCondition(condition)) continue;
		signals.push({
			id: `condition:${condition.type}`,
			label: "Condition",
			value: conditionSignalValue(condition),
			tone: condition.status === "False" ? "error" : "warning",
			source: "condition",
		});
	}

	const warningSummary = eventWarningSummary(events);
	if (warningSummary) {
		signals.push({
			id: "events:warnings",
			label: "Warning events",
			value: warningSummary,
			tone: "warning",
			source: "event",
		});
	}

	return signals;
}

export const formatMetadata = (
	metadata: Record<string, unknown>,
): Array<{ key: string; value: unknown }> => {
	const entries: Array<{ key: string; value: unknown }> = [];
	if (metadata.name) entries.push({ key: "Name", value: metadata.name });
	if (metadata.namespace)
		entries.push({ key: "Namespace", value: metadata.namespace });
	if (metadata.uid) entries.push({ key: "UID", value: metadata.uid });
	if (metadata.resourceVersion)
		entries.push({
			key: "Resource Version",
			value: metadata.resourceVersion,
		});
	if (metadata.creationTimestamp)
		entries.push({ key: "Created", value: metadata.creationTimestamp });
	if (metadata.labels)
		entries.push({
			key: "Labels",
			value: metadata.labels,
		});
	if (metadata.annotations)
		entries.push({
			key: "Annotations",
			value: metadata.annotations,
		});
	return entries;
};

export const getErrorMessage = (err: unknown): string => {
	if (err instanceof Error) return err.message;
	if (typeof err === "string") return err;
	return "Unknown error";
};
