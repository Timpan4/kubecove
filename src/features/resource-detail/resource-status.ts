import type { ResourceSummary } from "../../lib/types";
import type { ChipVariant } from "./constants";
import type { ConditionRow } from "./helpers";

function normalized(value: string | undefined): string {
	return value?.trim().toLowerCase() ?? "";
}

export function isSuccessfulTerminalResource(
	resource: Pick<ResourceSummary, "kind" | "status">,
): boolean {
	return (
		resource.kind === "Pod" &&
		["succeeded", "complete", "completed"].includes(normalized(resource.status))
	);
}

export function resourceReadyLabel(
	resource: Pick<ResourceSummary, "kind" | "status" | "ready">,
): string | undefined {
	const ready = normalized(resource.ready);
	if (ready === "true") return "Ready";
	if (ready === "false") {
		return isSuccessfulTerminalResource(resource) ? "Completed" : "Not ready";
	}
	return resource.ready;
}

export function resourceReadyTone(
	resource: Pick<ResourceSummary, "kind" | "status" | "ready">,
): ChipVariant {
	const ready = normalized(resource.ready);
	if (ready === "true") return "success";
	if (ready === "false") {
		return isSuccessfulTerminalResource(resource) ? "success" : "error";
	}
	return "neutral";
}

function isExpectedCompletedPodCondition(condition: ConditionRow): boolean {
	const type = normalized(condition.type);
	const reason = normalized(condition.reason);
	return (
		reason === "podcompleted" ||
		type === "podreadytostartcontainers" ||
		type === "ready" ||
		type === "containersready"
	);
}

function isFailureCondition(condition: ConditionRow): boolean {
	const type = normalized(condition.type);
	const reason = normalized(condition.reason);
	return (
		type === "failed" ||
		type === "failuretarget" ||
		reason.includes("failed") ||
		reason.includes("failure") ||
		reason === "backofflimitexceeded" ||
		reason === "deadlineexceeded"
	);
}

export function conditionStatusTone(
	condition: ConditionRow,
	resource: Pick<ResourceSummary, "kind" | "status">,
): ChipVariant {
	if (condition.status === "True") {
		return isFailureCondition(condition) ? "warning" : "success";
	}
	if (condition.status === "False") {
		return isSuccessfulTerminalResource(resource) &&
			isExpectedCompletedPodCondition(condition)
			? "neutral"
			: "error";
	}
	return "warning";
}
