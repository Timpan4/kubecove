/**
 * Pure extraction of the human-relevant parts of an Argo CD Application
 * `.status` blob, so the detail panel can explain *why* an app is Degraded
 * instead of dumping raw JSON.
 */

import type { JsonObject, JsonValue } from "@/lib/types";

export interface ArgoStatusCondition {
	type: string;
	message: string | null;
	lastTransitionTime: string | null;
}

export interface ArgoResourceHealth {
	kind: string | null;
	name: string | null;
	namespace: string | null;
	health: string | null;
	message: string | null;
	syncStatus: string | null;
}

export interface ArgoStatusInsights {
	healthMessage: string | null;
	healthTransitionTime: string | null;
	conditions: ArgoStatusCondition[];
	unhealthyResources: ArgoResourceHealth[];
}

function asRecord<Value>(value: Value): (Value & JsonObject) | null {
	return isRecord(value) ? value : null;
}

function asString(value: JsonValue | undefined): string | null {
	return String(value) === value && value.length > 0 ? value : null;
}

function isRecord<Value>(value: Value): value is Value & JsonObject {
	return value !== null && !Array.isArray(value) && Object(value) === value;
}

export function extractArgoStatusInsights(
	status: JsonObject | null | undefined,
): ArgoStatusInsights {
	const health = asRecord(status?.health);
	const conditions: ArgoStatusCondition[] = [];
	if (Array.isArray(status?.conditions)) {
		for (const item of status.conditions) {
			const condition = asRecord(item);
			const type = asString(condition?.type);
			if (!type) continue;
			conditions.push({
				type,
				message: asString(condition?.message),
				lastTransitionTime: asString(condition?.lastTransitionTime),
			});
		}
	}

	const unhealthyResources: ArgoResourceHealth[] = [];
	if (Array.isArray(status?.resources)) {
		for (const item of status.resources) {
			const resource = asRecord(item);
			if (!resource) continue;
			const resourceHealth = asRecord(resource.health);
			const healthStatus = asString(resourceHealth?.status);
			if (!healthStatus || healthStatus === "Healthy") continue;
			unhealthyResources.push({
				kind: asString(resource.kind),
				name: asString(resource.name),
				namespace: asString(resource.namespace),
				health: healthStatus,
				message: asString(resourceHealth?.message),
				syncStatus: asString(resource.status),
			});
		}
	}

	return {
		healthMessage: asString(health?.message),
		healthTransitionTime: asString(health?.lastTransitionTime),
		conditions,
		unhealthyResources,
	};
}
