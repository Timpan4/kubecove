import type {
	IncidentCockpitItem,
	ResourceDetailsFull,
	ResourceSummary,
	ResourceTopology,
} from "@/lib/types";
import {
	buildIncidentAvailableActions,
	buildIncidentGuidance,
	resolveIncidentOwner,
} from "./guidance";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toContain(expected: unknown): void;
};

function resource(kind: string, name: string): ResourceSummary {
	return {
		cluster: "kind-dev",
		kind,
		name,
		namespace: "payments",
		age: "1h",
		health: kind === "Pod" ? "degraded" : "healthy",
	};
}

function incident(subject = resource("Pod", "api-123")): IncidentCockpitItem {
	return {
		resource: subject,
		severity: "degraded",
		signals: [{ kind: "status", label: "Failed", message: "Ready False", source: "status" }],
		warningEventCount: 0,
	};
}

function topology(
	resources: ResourceSummary[],
	edges: Array<[number, number]>,
	warnings: string[] = [],
): ResourceTopology {
	return {
		nodes: resources.map((summary, index) => ({
			id: String(index),
			kind: summary.kind,
			name: summary.name,
			namespace: summary.namespace,
			health: summary.health,
			selectable: true,
			summary,
		})),
		edges: edges.map(([source, target], index) => ({
			id: String(index),
			source: String(source),
			target: String(target),
			relation: "owns",
		})),
		warnings,
	};
}

describe("incident guidance", () => {
	test("resolves Pod through ReplicaSet to Deployment", () => {
		const pod = resource("Pod", "api-123");
		const replicaSet = resource("ReplicaSet", "api-abc");
		const deployment = resource("Deployment", "api");
		const result = resolveIncidentOwner(pod, topology([deployment, replicaSet, pod], [[0, 1], [1, 2]]));

		expect(result.directOwner).toEqual(replicaSet);
		expect(result.workloadOwner).toEqual(deployment);
		expect(result.chain.map((entry) => entry.kind)).toEqual(["ReplicaSet", "Deployment"]);
		expect(result.subjectFound).toBe(true);
		expect(result.complete).toBe(true);
	});

	test("guards cycles and leaves unsupported owners non-actionable", () => {
		const pod = resource("Pod", "job-123");
		const job = resource("Job", "job");
		const result = resolveIncidentOwner(pod, topology([job, pod], [[0, 1], [1, 0]]));

		expect(result.directOwner).toEqual(job);
		expect(result.workloadOwner).toBe(null);
	});

	test("offers exact Pod delete plus supported owner actions", () => {
		const pod = resource("Pod", "api-123");
		const deployment = resource("Deployment", "api");
		const actions = buildIncidentAvailableActions(
			incident(pod),
			{
				directOwner: deployment,
				workloadOwner: deployment,
				chain: [deployment],
				subjectFound: true,
				complete: true,
			},
			"ready",
		);

		expect(actions.map((action) => action.label)).toEqual([
			"Recreate this Pod",
			"Restart owning Deployment",
			"Scale owning Deployment",
		]);
		expect(actions.every((action) => action.target === pod || action.target === deployment)).toBe(true);
	});

	test("labels standalone Pod deletion without promising recreation", () => {
		const actions = buildIncidentAvailableActions(incident(), {
			directOwner: null,
			workloadOwner: null,
			chain: [],
			subjectFound: true,
			complete: true,
		}, "ready");

		expect(actions[0]?.label).toBe("Delete this Pod");
		expect(actions[0]?.description).toContain("will not be recreated automatically");
	});

	test("offers StatefulSet and DaemonSet owner actions without inventing scale", () => {
		const pod = resource("Pod", "api-0");
		const statefulSet = resource("StatefulSet", "api");
		const daemonSet = resource("DaemonSet", "agent");
		const statefulActions = buildIncidentAvailableActions(
			incident(pod),
			resolveIncidentOwner(pod, topology([statefulSet, pod], [[0, 1]])),
			"ready",
		);
		const daemonActions = buildIncidentAvailableActions(
			incident(pod),
			resolveIncidentOwner(pod, topology([daemonSet, pod], [[0, 1]])),
			"ready",
		);

		expect(statefulActions.map((action) => action.label)).toEqual([
			"Recreate this Pod",
			"Restart owning StatefulSet",
			"Scale owning StatefulSet",
		]);
		expect(daemonActions.map((action) => action.label)).toEqual([
			"Recreate this Pod",
			"Restart owning DaemonSet",
		]);
	});

	test("does not expose unsupported owner actions", () => {
		const pod = resource("Pod", "job-123");
		const job = resource("Job", "job");
		const actions = buildIncidentAvailableActions(
			incident(pod),
			resolveIncidentOwner(pod, topology([job, pod], [[0, 1]])),
			"ready",
		);

		expect(actions.map((action) => action.label)).toEqual(["Delete this Pod"]);
		expect(actions[0]?.description).toContain("not a supported restart or scale target");
	});

	test("treats missing subjects and dangling owner nodes as incomplete", () => {
		const pod = resource("Pod", "api-123");
		const deployment = resource("Deployment", "api");
		const missingSubject = resolveIncidentOwner(pod, topology([deployment], []));
		const missingOwner = resolveIncidentOwner(pod, topology([pod], [[9, 0]]));

		expect(missingSubject.subjectFound).toBe(false);
		expect(missingSubject.complete).toBe(false);
		expect(missingOwner.subjectFound).toBe(true);
		expect(missingOwner.complete).toBe(false);
		expect(buildIncidentAvailableActions(incident(pod), missingOwner, "ready")[0]?.description)
			.toContain("automatic replacement is not confirmed");
	});

	test("withholds cached owner actions while topology is loading or failed", () => {
		const pod = resource("Pod", "api-123");
		const deployment = resource("Deployment", "api");
		const resolution = resolveIncidentOwner(pod, topology([deployment, pod], [[0, 1]]));

		for (const state of ["loading", "error"] as const) {
			const actions = buildIncidentAvailableActions(incident(pod), resolution, state);
			expect(actions.map((action) => action.label)).toEqual(["Delete this Pod"]);
			expect(actions[0]?.description).toContain("automatic replacement is not confirmed");
		}
	});

	test("withholds owner actions when topology reports warnings", () => {
		const pod = resource("Pod", "api-123");
		const deployment = resource("Deployment", "api");
		const resolution = resolveIncidentOwner(
			pod,
			topology([deployment, pod], [[0, 1]], ["partial topology"]),
		);

		expect(resolution.workloadOwner).toEqual(deployment);
		expect(resolution.complete).toBe(false);
		expect(buildIncidentAvailableActions(incident(pod), resolution, "ready").map((action) => action.label))
			.toEqual(["Delete this Pod"]);
	});

	test("keeps unknown root cause explicit when detail enrichment is unavailable", () => {
		const guidance = buildIncidentGuidance(
			incident(),
			undefined,
			{
				directOwner: null,
				workloadOwner: null,
				chain: [],
				subjectFound: false,
				complete: false,
			},
			"error",
			"error",
		);

		expect(guidance.missing[0]).toBe("Root cause is not confirmed by current evidence.");
		expect(guidance.evidence[0]?.label).toBe("Failed");
	});

	test("merges live detail and snapshot evidence without duplicating warning events", () => {
		const item = incident();
		item.signals = [
			{
				kind: "warning-event",
				label: "BackOff",
				message: "Back-off restarting container",
				source: "events",
				lastSeenAt: "2026-07-16T09:00:00Z",
			},
			{ kind: "status", label: "Failed", message: "Ready False", source: "status" },
		];
		item.latestWarningEvent = {
			eventType: "Warning",
			reason: "BackOff",
			message: "Back-off restarting container",
			count: 2,
			lastSeen: "1m",
			lastSeenAt: "2026-07-16T09:00:00Z",
			source: "kubelet",
			namespace: "payments",
		};
		const details: ResourceDetailsFull = {
			summary: item.resource,
			yaml: "",
			metadata: {},
			status: {
				containerStatuses: [{
					name: "api",
					ready: false,
					restartCount: 0,
					state: { waiting: { reason: "CrashLoopBackOff" } },
				}],
			},
		};
		const guidance = buildIncidentGuidance(
			item,
			details,
			{ directOwner: null, workloadOwner: null, chain: [], subjectFound: true, complete: true },
			"ready",
			"ready",
		);

		expect(guidance.evidence.map((entry) => entry.label)).toEqual([
			"api waiting",
			"BackOff",
			"Failed",
		]);
		expect(guidance.evidence.filter((entry) => entry.label === "BackOff").length).toBe(1);
	});
});
