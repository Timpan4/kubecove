import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import { buildIncidentGuidance } from "@/features/incidents/guidance";
import {
	buildIncidentAvailableActions,
	resolveIncidentOwner,
} from "@/features/incidents/incident-actions";
import {
	buildIncidentSurfaceState,
	incidentItemKey,
} from "@/features/incidents/model";
import type {
	IncidentCockpitItem,
	ResourceDetailsFull,
	ResourceTopology,
	TopologyNode,
} from "@/lib/types";
import { buildOwnershipTopology } from "./topology-benchmark-fixtures";
import { workspaceRows } from "./workspace-benchmark-fixtures";

const incidentItems: IncidentCockpitItem[] = workspaceRows.map(
	(resource, index) => {
		const severity =
			index % 4 === 0
				? "degraded"
				: index % 4 === 1
					? "attention"
					: index % 4 === 2
						? "restarted"
						: "warning";
		const state =
			index % 7 === 0
				? "resolved"
				: severity === "restarted"
					? "historical"
					: "active";
		const incident: IncidentCockpitItem = {
			resource,
			severity,
			state,
			signals: [
				{
					kind: severity === "restarted" ? "restart" : "condition",
					label: `Signal ${index % 8}`,
					message: `Synthetic incident evidence ${index}`,
					source: "kubernetes",
					state,
					lastSeenAt: `2026-10-${String((index % 9) + 1).padStart(2, "0")}T12:00:00Z`,
				},
				...(index % 3 === 0
					? [
							{
								kind: "event",
								label: "Warning event",
								message: `Warning details ${index}`,
								source: "event",
								state: "active" as const,
							},
						]
					: []),
			],
			warningEventCount: index % 3 === 0 ? 2 : 0,
			latestSignalAt: `2026-10-${String((index % 9) + 1).padStart(2, "0")}T12:00:00Z`,
		};
		if (index % 3 === 0) {
			incident.latestWarningEvent = {
				eventType: "Warning",
				reason: "BackOff",
				message: `Container restart ${index}`,
				count: 2,
				lastSeen: "1m",
				lastSeenAt: "2026-10-11T12:00:00Z",
				source: "kubelet",
				namespace: resource.namespace ?? null,
			};
		}
		return incident;
	},
);
const topology = buildOwnershipTopology(500);
const selectedIncident = incidentItems.find(
	(item) => item.resource.kind === "Pod",
);
if (!selectedIncident) throw new Error("Missing selected Pod incident fixture");
const selectedTopologyNode = topology.nodes[1];
if (!selectedTopologyNode)
	throw new Error("Missing selected Pod topology fixture");
const selectedPod = selectedTopologyNode.summary;
const ownerIncident: IncidentCockpitItem = {
	...selectedIncident,
	resource: selectedPod,
};
const workloadNode = topology.nodes[0];
assert(workloadNode && workloadNode.kind === "Deployment");
const replicaNode: TopologyNode = {
	...selectedTopologyNode,
	id: "incident-replicaset",
	kind: "ReplicaSet",
	name: "replica-controller",
	summary: {
		...selectedPod,
		apiVersion: "apps/v1",
		kind: "ReplicaSet",
		name: "replica-controller",
	},
};
const dynamicNode: TopologyNode = {
	...selectedTopologyNode,
	id: "incident-widget",
	kind: "Widget",
	name: "a-widget",
	summary: {
		...selectedPod,
		apiVersion: "operator.example/v1",
		kind: "Widget",
		name: "a-widget",
	},
};
const lookalikeNode: TopologyNode = {
	...selectedTopologyNode,
	id: "incident-lookalike",
	summary: { ...selectedPod, apiVersion: "foreign.example/v1" },
};
const ownershipTopology: ResourceTopology = {
	...topology,
	nodes: [lookalikeNode, ...topology.nodes, replicaNode, dynamicNode],
	edges: [
		...topology.edges.filter((edge) => edge.target !== selectedTopologyNode.id),
		{
			id: "incident-deployment-rs",
			source: workloadNode.id,
			target: replicaNode.id,
			relation: "owns",
		},
		{
			id: "incident-rs-pod",
			source: replicaNode.id,
			target: selectedTopologyNode.id,
			relation: "owns",
		},
		{
			id: "incident-widget-pod",
			source: dynamicNode.id,
			target: selectedTopologyNode.id,
			relation: "owns",
		},
		{
			id: "incident-widget-rs",
			source: dynamicNode.id,
			target: replicaNode.id,
			relation: "owns",
		},
		{
			id: "incident-rs-widget",
			source: replicaNode.id,
			target: dynamicNode.id,
			relation: "owns",
		},
	],
};
const incompleteTopology: ResourceTopology = {
	...ownershipTopology,
	edges: [
		...ownershipTopology.edges,
		{
			id: "incident-dangling",
			source: "missing-owner",
			target: dynamicNode.id,
			relation: "owns",
		},
	],
};
const ownerResolution = resolveIncidentOwner(selectedPod, ownershipTopology);
const details: ResourceDetailsFull = {
	summary: selectedPod,
	yaml: "apiVersion: v1\nkind: Pod\nmetadata:\n  name: api-0-pod\n",
	metadata: {},
	status: {
		phase: "Running",
		conditions: [
			{
				type: "Ready",
				status: "False",
				reason: "ContainersNotReady",
				message: "Container is restarting",
			},
		],
		containerStatuses: [
			{
				name: "api",
				ready: false,
				restartCount: 4,
				state: {
					waiting: { reason: "CrashLoopBackOff", message: "Restart backoff" },
				},
			},
		],
	},
};
// Detail scans happen before the six-row evidence presentation bound.
assert(details.status);
details.status.conditions = Array.from({ length: 12 }, (_, index) => ({
	type: `Condition${index}`,
	status: "False",
	reason: "Unavailable",
	message: `Detail evidence ${index}`,
}));
details.status.containerStatuses = Array.from({ length: 20 }, (_, index) => ({
	name: `container-${index}`,
	ready: false,
	restartCount: index,
	state: {
		waiting: {
			reason: "CrashLoopBackOff",
			message: `Container evidence ${index}`,
		},
	},
}));
const detailHeavyItem: IncidentCockpitItem = {
	...ownerIncident,
	severity: "degraded",
	state: "active",
	signals: Array.from({ length: 10 }, (_, index) => ({
		kind: index === 0 ? "restart" : "condition",
		label: `Evidence ${index}`,
		message: `Snapshot evidence detail ${index}`,
		source: "kubernetes",
		state: "active",
		lastSeenAt: "2026-10-11T12:00:00Z",
	})),
	warningEventCount: 1,
};
const restartSignal = detailHeavyItem.signals[0];
assert(restartSignal);
detailHeavyItem.signals.unshift({
	...restartSignal,
	label: ` ${restartSignal.label.toUpperCase()} `,
	message: ` ${restartSignal.message.toUpperCase()} `,
});
const firstIncident = incidentItems[0];
const fortyThirdIncident = incidentItems[42];
if (!firstIncident || !fortyThirdIncident)
	throw new Error("Missing incident selection fixtures");
const selectedKey = incidentItemKey(firstIncident);
const collisionItem = {
	...firstIncident,
	resource: { ...firstIncident.resource, apiVersion: "foreign.example/v1" },
};
const collisionItems = [...incidentItems, collisionItem];
const disappearedItems = incidentItems.filter(
	(item) => incidentItemKey(item) !== selectedKey,
);
const collisionState = buildIncidentSurfaceState(
	collisionItems,
	"all",
	selectedKey,
);
const disappearedState = buildIncidentSurfaceState(
	disappearedItems,
	"all",
	selectedKey,
);
assert.notEqual(incidentItemKey(collisionItem), selectedKey);
assert.equal(collisionState.counts.total, 10_001);
assert.equal(
	collisionState.groups.reduce((count, group) => count + group.items.length, 0),
	collisionState.visibleCount,
);
assert.notEqual(disappearedState.selectedKey, selectedKey);
assert.equal(
	disappearedState.selectedIncident,
	disappearedState.visibleItems[0],
);
const incompleteResolution = resolveIncidentOwner(
	selectedPod,
	incompleteTopology,
);
assert.equal(incompleteResolution.complete, false);
assert.equal(
	buildIncidentAvailableActions(
		ownerIncident,
		incompleteResolution,
		"ready",
	).some((action) => action.id.startsWith("owner:")),
	false,
);
const historicalItem: IncidentCockpitItem = {
	...detailHeavyItem,
	state: "historical",
	severity: "restarted",
	signals: detailHeavyItem.signals.map((signal) => ({
		...signal,
		state: "historical",
	})),
};
assert.equal(
	buildIncidentGuidance(
		historicalItem,
		details,
		ownerResolution,
		"ready",
		"ready",
	).actions.length,
	0,
);
assert(
	buildIncidentGuidance(
		detailHeavyItem,
		undefined,
		ownerResolution,
		"error",
		"error",
	).missing.some((message) => message.includes("could not be loaded")),
);

assert.equal(incidentItems.length, 10_000);
assert.equal(topology.nodes.length, 1_000);
assert.equal(ownerResolution.workloadOwner?.kind, "Deployment");
assert.equal(ownerResolution.chain.length, 2);
assert.equal(ownerResolution.subjectFound, true);
const guidanceEvidence = buildIncidentGuidance(
	detailHeavyItem,
	details,
	ownerResolution,
	"ready",
	"ready",
).evidence;
assert.equal(guidanceEvidence.length, 6);
assert.equal(
	new Set(
		guidanceEvidence.map(
			(entry) =>
				`${entry.label.trim().toLowerCase()}\u0000${entry.detail.trim().toLowerCase()}\u0000${entry.timestamp ?? ""}`,
		),
	).size,
	guidanceEvidence.length,
);

describe("incident surface and selected enrichment F51–F53", () => {
	bench(
		"F51 cold incident counts, filtering, sorting, grouping, and selection",
		() => {
			buildIncidentSurfaceState(collisionItems, "all", selectedKey);
		},
	);

	bench("F51 retained incident filter and selection updates", () => {
		buildIncidentSurfaceState(incidentItems, "degraded", selectedKey);
		buildIncidentSurfaceState(
			incidentItems,
			"warning",
			incidentItemKey(fortyThirdIncident),
		);
		buildIncidentSurfaceState(disappearedItems, "all", selectedKey);
	});

	bench(
		"F52 topology arrival, owner resolution, and guarded action projection",
		() => {
			const resolved = resolveIncidentOwner(selectedPod, ownershipTopology);
			buildIncidentAvailableActions(ownerIncident, resolved, "ready");
			const incomplete = resolveIncidentOwner(selectedPod, incompleteTopology);
			buildIncidentAvailableActions(ownerIncident, incomplete, "ready");
		},
	);

	bench(
		"F53 detail-ready evidence enrichment and guidance output bound",
		() => {
			buildIncidentGuidance(
				detailHeavyItem,
				details,
				ownerResolution,
				"ready",
				"ready",
			);
			buildIncidentGuidance(
				detailHeavyItem,
				undefined,
				ownerResolution,
				"loading",
				"loading",
			);
			buildIncidentGuidance(
				detailHeavyItem,
				undefined,
				incompleteResolution,
				"error",
				"error",
			);
			buildIncidentGuidance(
				historicalItem,
				details,
				ownerResolution,
				"ready",
				"ready",
			);
		},
	);
});
