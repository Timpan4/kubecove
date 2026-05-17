import type { ResourceSummary } from "./types";

export interface ResourceHealthFlags {
	healthy: boolean;
	attention: boolean;
	degraded: boolean;
	restarted: boolean;
}

export function classifyResourceHealth(row: ResourceSummary): ResourceHealthFlags {
	const status = row.status?.toLowerCase() ?? "";
	const ready = row.ready?.toLowerCase() ?? "";
	const restarts = row.restarts ?? 0;
	const restarted = restarts > 0;
	const degraded =
		status === "failed" ||
		status === "error" ||
		status === "crashloopbackoff" ||
		status === "imagepullbackoff" ||
		ready === "false";
	const attention =
		!degraded &&
		(restarted ||
			status === "pending" ||
			status === "terminating" ||
			status === "unknown");
	const healthy =
		!degraded &&
		!attention &&
		(status === "running" ||
			status === "succeeded" ||
			status === "ready" ||
			ready === "true");

	return {
		healthy,
		attention,
		degraded,
		restarted,
	};
}
