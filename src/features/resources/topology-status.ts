import type { TopologyStoplightTone } from "./topology-types";

const SUCCESS_STATUSES = new Set([
	"available",
	"complete",
	"completed",
	"ready",
	"running",
	"succeeded",
]);
const WARNING_STATUSES = new Set([
	"pending",
	"restarting",
	"terminating",
	"unknown",
	"waiting",
]);
const ERROR_STATUSES = new Set([
	"crashloopbackoff",
	"degraded",
	"error",
	"failed",
	"imagepullbackoff",
	"not ready",
	"notready",
]);
const TONE_RANK: Record<TopologyStoplightTone, number> = {
	neutral: 0,
	success: 1,
	warning: 2,
	error: 3,
};

function normalized(value: string | undefined): string {
	return value?.trim().toLowerCase() ?? "";
}

function isTerminalSuccessStatus(status: string | undefined): boolean {
	const value = normalized(status);
	return value === "complete" || value === "completed" || value === "succeeded";
}

export function topologyStatusTone(status: string | undefined): TopologyStoplightTone {
	const value = normalized(status);
	if (SUCCESS_STATUSES.has(value)) return "success";
	if (WARNING_STATUSES.has(value)) return "warning";
	if (ERROR_STATUSES.has(value)) return "error";
	return "neutral";
}

function topologyHealthTone(health: string | undefined): TopologyStoplightTone {
	const value = normalized(health);
	if (value === "healthy") return "success";
	if (value === "degraded") return "error";
	if (value === "attention") return "warning";
	return "neutral";
}

export function topologyRestartTone(restarts: number | undefined): TopologyStoplightTone {
	if (restarts === undefined || restarts <= 0) return "neutral";
	if (restarts >= 5) return "error";
	if (restarts >= 3) return "warning";
	return "neutral";
}

export function topologyReadyTone(
	ready: string | undefined,
	status: string | undefined,
): TopologyStoplightTone {
	const value = normalized(ready);
	if (!value) return "neutral";
	if (isTerminalSuccessStatus(status) && (value === "false" || value === "not ready")) {
		return "success";
	}
	if (value === "true" || value === "ready" || value === "completed") return "success";
	if (value === "false" || value === "not ready" || value === "notready") return "error";

	const ratioMatch = /^(\d+)\s*\/\s*(\d+)$/.exec(value);
	if (!ratioMatch) return "neutral";
	const readyCount = Number(ratioMatch[1]);
	const desiredCount = Number(ratioMatch[2]);
	if (desiredCount > 0 && readyCount >= desiredCount) return "success";
	return topologyStatusTone(status) === "warning" ? "warning" : "error";
}

export function topologyReadyText(
	ready: string | undefined,
	status: string | undefined,
): string | undefined {
	if (!ready) return undefined;
	const value = normalized(ready);
	if (isTerminalSuccessStatus(status) && (value === "false" || value === "not ready")) {
		return "Completed";
	}
	if (value === "true") return "Ready";
	if (value === "false") return "Not ready";
	return ready;
}

export function topologyRailTone(
	status: string | undefined,
	ready: string | undefined,
	health?: string | undefined,
): TopologyStoplightTone {
	return [
		topologyStatusTone(status),
		topologyReadyTone(ready, status),
		topologyHealthTone(health),
	].sort((a, b) => TONE_RANK[b] - TONE_RANK[a])[0];
}
