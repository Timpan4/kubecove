import type {
	HealthAssessment,
	ResourceKindSelection,
	ResourceSummary,
} from "@/lib/types";
import { entryPointFromResource } from "@/lib/workspace-entry-points";
import type { SavedWorkspace, WorkspaceScope } from "@/lib/workspace-model";

const kindNames = [
	"Pod",
	"Deployment",
	"ReplicaSet",
	"Service",
	"ConfigMap",
	"Secret",
];
const healthStates: HealthAssessment["state"][] = [
	"healthy",
	"needsAttention",
	"degraded",
	"unknown",
	"notEvaluated",
];

function assessment(state: HealthAssessment["state"]): HealthAssessment {
	const source = state === "notEvaluated" ? "provider" : "kubernetes";
	return {
		state,
		completeness: state === "unknown" ? "partial" : "complete",
		winningSources: state === "notEvaluated" ? [] : [source],
		reasons: [`Synthetic ${state} evidence`],
		evidence: [
			{
				source,
				raw: { state },
				state,
				current: true,
				reason: `Synthetic ${state}`,
			},
		],
	};
}

export const workspaceRows: ResourceSummary[] = Array.from(
	{ length: 10_000 },
	(_, index) => {
		const kind = kindNames[index % kindNames.length] ?? "Pod";
		const cluster = `prod/context-${index % 3}`;
		const namespace = `namespace-${index % 100}`;
		const state = healthStates[index % healthStates.length] ?? "unknown";
		return {
			cluster,
			apiVersion:
				kind === "Pod" ||
				kind === "Service" ||
				kind === "ConfigMap" ||
				kind === "Secret"
					? "v1"
					: "apps/v1",
			kind,
			name: `resource-${index}`,
			namespace,
			age: `${index % 60}m`,
			health:
				state === "needsAttention"
					? "attention"
					: state === "notEvaluated"
						? "unknown"
						: state,
			healthAssessment: assessment(state),
			restarts: index % 100 === 0 ? 3 : index % 4,
			status: state === "degraded" ? "CrashLoopBackOff" : "Running",
		};
	},
);

const workspaceKinds: ResourceKindSelection[] = [
	"Pod",
	"Deployment",
	"ReplicaSet",
	"Service",
	"ConfigMap",
	"Secret",
];

export function workspaceScope(index = 0): WorkspaceScope {
	const clusterContext = `prod/context-${index % 3}`;
	return {
		clusterContext,
		clusterGroup: {
			id: "production",
			name: "Production",
			members: ["prod/context-0", "prod/context-1", "prod/context-2"],
		},
		namespaces: Array.from(
			{ length: 100 },
			(_, namespace) => `namespace-${namespace}`,
		),
		kinds: workspaceKinds,
		argoAppFilter: "",
		layout: "overview",
	};
}

export function savedWorkspace(
	index: number,
	pinnedCount = 100,
): SavedWorkspace {
	const pinned = Array.from({ length: pinnedCount }, (_, entryIndex) => {
		const row =
			workspaceRows[(index * pinnedCount + entryIndex) % workspaceRows.length];
		if (!row) throw new Error(`Missing pinned resource fixture ${entryIndex}`);
		return entryPointFromResource(
			row,
			`2026-01-${String((entryIndex % 28) + 1).padStart(2, "0")}T00:00:00Z`,
		);
	});
	const recent = Array.from({ length: 8 }, (_, entryIndex) => {
		const row = workspaceRows[(index * 8 + entryIndex) % workspaceRows.length];
		if (!row) throw new Error(`Missing recent resource fixture ${entryIndex}`);
		return entryPointFromResource(
			row,
			`2026-02-${String(entryIndex + 1).padStart(2, "0")}T00:00:00Z`,
		);
	});
	return {
		id: `workspace-${index}`,
		name: `Workspace ${index}`,
		createdAt: "2026-01-01T00:00:00Z",
		updatedAt: "2026-01-02T00:00:00Z",
		scope: workspaceScope(index),
		shortcuts: [],
		portForwards: [],
		entryPoints: { pinned, recent },
	};
}
