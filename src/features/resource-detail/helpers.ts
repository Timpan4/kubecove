import type {
	DiscoveredResourceKind,
	ResourceEventSummary,
	ResourceSummary,
} from "../../lib/types";
import type { ChipVariant } from "./constants";
import { isSuccessfulTerminalResource } from "./resource-status";

export { incidentSignalCardClassName } from "./incident-signal-styles";
export {
	conditionStatusTone,
	resourceReadyLabel,
	resourceReadyTone,
} from "./resource-status";

export interface ConditionRow {
	type: string;
	status: string;
	reason?: string;
	message?: string;
	lastTransitionTime?: string;
}

export interface ContainerStatusRow {
	name: string;
	type?: "init" | "container" | "ephemeral";
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
	const containers = [
		...getStatusList(status, "initContainerStatuses").map((container) => ({
			container,
			type: "init" as const,
		})),
		...getStatusList(status, "containerStatuses").map((container) => ({
			container,
			type: "container" as const,
		})),
		...getStatusList(status, "ephemeralContainerStatuses").map((container) => ({
			container,
			type: "ephemeral" as const,
		})),
	];
	return containers.map(({ container, type }) => {
		const currentState = parseStateFields(container.state);
		const lastState = parseStateFields(container.lastState);
		return {
			name: String(container.name ?? "container"),
			type,
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
	valueParts?: IncidentSignalValuePart[];
	tone: ChipVariant;
	source: "status" | "condition" | "event" | "container";
}

export type IncidentSignalValuePart =
	| { kind: "text"; text: string }
	| { kind: "timestamp"; value: string };

interface IncidentSignalOptions {
	now?: Date;
	staleRestartMs?: number;
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
	if (!timestamp) return false;
	const value = Date.parse(timestamp);
	if (Number.isNaN(value)) return false;
	return now.getTime() - value <= staleRestartMs;
}

function normalized(value: string | undefined): string {
	return value?.trim().toLowerCase() ?? "";
}

export function resourceStatusTone(status: string | undefined): ChipVariant {
	const value = normalized(status);
	if (["running", "succeeded", "complete", "completed", "ready"].includes(value)) {
		return "success";
	}
	if (["failed", "error", "crashloopbackoff", "imagepullbackoff"].includes(value)) {
		return "error";
	}
	if (["pending", "terminating", "unknown"].includes(value)) return "warning";
	return "neutral";
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

function hasPreviousRestartContext(container: ContainerStatusRow): boolean {
	return (
		container.lastState !== undefined ||
		container.lastReason !== undefined ||
		container.lastMessage !== undefined ||
		container.lastStartedAt !== undefined ||
		container.lastFinishedAt !== undefined ||
		container.lastExitCode !== undefined
	);
}

function isCleanPreviousRestart(container: ContainerStatusRow): boolean {
	if (container.lastState !== "terminated") return false;
	if (container.lastExitCode !== undefined && container.lastExitCode !== 0) {
		return false;
	}
	return container.lastReason === "Completed" || container.lastExitCode === 0;
}

function isActionableContainerRestart(
	container: ContainerStatusRow,
	now: Date,
	staleRestartMs: number,
	succeededResource: boolean,
): boolean {
	if (container.restartCount <= 0) return false;
	if (succeededResource && isCleanCompletedContainer(container)) return false;
	if (
		container.type === "init" &&
		isCleanCompletedContainer(container) &&
		isCleanPreviousRestart(container) &&
		!isRecentTimestamp(container.lastFinishedAt, now, staleRestartMs)
	) {
		return false;
	}
	if (container.ready === false) return true;
	if (container.state === "waiting") return true;
	if (container.state === "terminated" && container.exitCode !== 0) return true;
	if (container.lastExitCode !== undefined && container.lastExitCode !== 0) return true;
	if (container.lastReason && container.lastReason !== "Completed") return true;
	if (isRecentTimestamp(container.lastFinishedAt, now, staleRestartMs)) {
		return true;
	}
	if (isCleanPreviousRestart(container)) return false;
	return !hasPreviousRestartContext(container);
}

function restartSignalValue(containers: ContainerStatusRow[]): string {
	return signalValueFromParts(restartSignalValueParts(containers));
}

function textPart(text: string): IncidentSignalValuePart {
	return { kind: "text", text };
}

function timestampPart(value: string): IncidentSignalValuePart {
	return { kind: "timestamp", value };
}

function signalValueFromParts(parts: IncidentSignalValuePart[]): string {
	return parts
		.map((part) => (part.kind === "text" ? part.text : part.value))
		.join("");
}

function restartSignalValueParts(
	containers: ContainerStatusRow[],
): IncidentSignalValuePart[] {
	const parts: IncidentSignalValuePart[] = [];
	containers.forEach((container, index) => {
		if (index > 0) parts.push(textPart("; "));
		parts.push(
			textPart(
				`${container.name} restarted ${container.restartCount} ${
					container.restartCount === 1 ? "time" : "times"
				}`,
			),
		);
		if (container.lastReason) parts.push(textPart(` · ${container.lastReason}`));
		if (container.lastExitCode !== undefined) {
			parts.push(textPart(` · exit ${container.lastExitCode}`));
		}
		if (container.lastFinishedAt) {
			parts.push(textPart(" · finished "));
			parts.push(timestampPart(container.lastFinishedAt));
		}
	});
	return parts;
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
	return signalValueFromParts(conditionSignalValueParts(condition));
}

function conditionSignalValueParts(
	condition: ConditionRow,
): IncidentSignalValuePart[] {
	const parts: IncidentSignalValuePart[] = [
		textPart(`${condition.type}=${condition.status}`),
	];
	if (condition.reason) parts.push(textPart(` · ${condition.reason}`));
	if (condition.lastTransitionTime) {
		parts.push(textPart(" · since "));
		parts.push(timestampPart(condition.lastTransitionTime));
	}
	return parts;
}

function isContainerIncident(container: ContainerStatusRow): boolean {
	if (isCleanCompletedContainer(container)) return false;
	if (container.state === "terminated" && container.exitCode !== 0) return true;
	if (container.state === "waiting") return true;
	return (
		container.ready === false &&
		(Boolean(container.state) ||
			Boolean(container.reason) ||
			Boolean(container.message))
	);
}

function containerIncidentTone(container: ContainerStatusRow): ChipVariant {
	const reason = normalized(container.reason);
	if (container.state === "terminated" && container.exitCode !== 0) return "error";
	if (container.ready === false && reason.includes("error")) return "error";
	if (container.ready === false && reason.includes("crash")) return "error";
	if (container.ready === false && container.state !== "waiting") return "error";
	return "warning";
}

function containerSignalLabel(container: ContainerStatusRow): string {
	return `${container.name} ${container.state ?? "not ready"}`;
}

function containerSignalValueParts(
	container: ContainerStatusRow,
): IncidentSignalValuePart[] {
	const parts: IncidentSignalValuePart[] = [];
	if (container.reason) parts.push(textPart(container.reason));
	if (container.message) {
		if (parts.length > 0) parts.push(textPart(" · "));
		parts.push(textPart(container.message));
	}
	if (container.exitCode !== undefined) {
		if (parts.length > 0) parts.push(textPart(" · "));
		parts.push(textPart(`exit ${container.exitCode}`));
	}
	if (container.finishedAt) {
		if (parts.length > 0) parts.push(textPart(" · "));
		parts.push(textPart("finished "));
		parts.push(timestampPart(container.finishedAt));
	}
	if (parts.length === 0) {
		parts.push(textPart(container.ready === false ? "not ready" : "state needs attention"));
	}
	return parts;
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
	const succeededResource = isSuccessfulTerminalResource(resource);

	for (const container of containers?.filter(isContainerIncident) ?? []) {
		const valueParts = containerSignalValueParts(container);
		signals.push({
			id: `container:${container.name}:${container.state ?? "not-ready"}`,
			label: containerSignalLabel(container),
			value: signalValueFromParts(valueParts),
			valueParts,
			tone: containerIncidentTone(container),
			source: "container",
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
		const restartValueParts =
			actionableRestartContainers && actionableRestartContainers.length > 0
				? restartSignalValueParts(actionableRestartContainers)
				: undefined;
		signals.push({
			id: "restarts",
			label: "Restarts",
			value: restartValue,
			valueParts: restartValueParts,
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
				valueParts: conditionSignalValueParts(condition),
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
			valueParts: conditionSignalValueParts(condition),
			tone: condition.status === "False" ? "error" : "warning",
			source: "condition",
		});
	}

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
		signals.push({
			id: "status",
			label: "Status",
			value: resource.status,
			tone: resourceStatusTone(resource.status),
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
	if (isRecord(err) && typeof err.message === "string") return err.message;
	return "Unknown error";
};
