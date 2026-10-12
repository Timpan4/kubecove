import type { JsonObject } from "./types";

export const OTHER_RESOURCE_KINDS = "*";

export const RESOURCE_KIND_LANE_IDS = ["first", "normal", "last"] as const;
export type ResourceKindLaneId = (typeof RESOURCE_KIND_LANE_IDS)[number];
export interface ResourceKindLanes {
	first: string[];
	normal: string[];
	last: string[];
}

export function defaultResourceKindLanes(): ResourceKindLanes {
	return {
		first: ["Pod", "Deployment", "StatefulSet", "DaemonSet"],
		normal: [
			"CronJob",
			"Job",
			"Service",
			"Ingress",
			"ConfigMap",
			"Secret",
			"PersistentVolumeClaim",
			OTHER_RESOURCE_KINDS,
		],
		last: ["ReplicaSet", "Node", "PersistentVolume", "StorageClass"],
	};
}

export function resourceKindOrder(lanes: ResourceKindLanes): string[] {
	return RESOURCE_KIND_LANE_IDS.flatMap((lane) => lanes[lane]);
}

/// Persisted lanes are untrusted: keep string kinds once each, in lane order,
/// and keep exactly one slot for unlisted kinds.
export function normalizeResourceKindLanes<Value>(value: Value): ResourceKindLanes {
	if (!isRecord(value)) return defaultResourceKindLanes();
	const seen = new Set<string>();
	const lanes: ResourceKindLanes = { first: [], normal: [], last: [] };
	for (const lane of RESOURCE_KIND_LANE_IDS) {
		const kinds = value[lane];
		if (!Array.isArray(kinds)) continue;
		for (const kind of kinds) {
			if (!isString(kind) || !kind.trim() || seen.has(kind)) continue;
			seen.add(kind);
			lanes[lane].push(kind);
		}
	}
	if (seen.size === 0) return defaultResourceKindLanes();
	if (!seen.has(OTHER_RESOURCE_KINDS)) lanes.normal.push(OTHER_RESOURCE_KINDS);
	return lanes;
}

function isRecord<Value>(value: Value): value is Value & JsonObject {
	return value !== null && !Array.isArray(value) && Object(value) === value;
}

function isString<Value>(value: Value): value is Value & string {
	return String(value) === value;
}
