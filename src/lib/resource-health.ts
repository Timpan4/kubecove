import type { ResourceSummary } from "./types";

export interface ResourceHealthFlags {
	healthy: boolean;
	attention: boolean;
	degraded: boolean;
	restarted: boolean;
}

export function classifyResourceHealth(row: ResourceSummary): ResourceHealthFlags {
	const restarts = row.restarts ?? 0;

	return {
		healthy: row.health === "healthy",
		attention: row.health === "attention",
		degraded: row.health === "degraded",
		restarted: row.health === "restarted" || restarts > 0,
	};
}
