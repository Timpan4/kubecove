import { describe, expect, test } from "bun:test";
import { buildOverviewModel } from "../src/features/workspaces/overview/model";
import type { HealthAssessmentState } from "../src/lib/health-types";
import type { ResourceSummary } from "../src/lib/types";

function row(kind: string, name: string, state: HealthAssessmentState = "degraded", extra: Partial<ResourceSummary> = {}): ResourceSummary {
	return { kind, name, uid: `${kind}/${name}`, apiVersion: "apps/v1", cluster: "dev", namespace: "apps", age: "1d", health: "unknown",
		healthAssessment: { state, completeness: "complete", winningSources: ["kubernetes"], reasons: [state], evidence: [] },
		...extra };
}
function owner(kind: string, name: string) { return { apiVersion: "apps/v1", kind, name, uid: `${kind}/${name}` }; }

describe("workspace overview findings", () => {
	test("namespaced dependents can fold into a cluster-scoped controller", () => {
		const controller = row("Controller", "global", "degraded", { namespace: null });
		const pod = row("Pod", "child", "degraded", { controllerOwner: owner("Controller", "global") });
		const model = buildOverviewModel([controller, pod]);
		expect(model.problems).toHaveLength(1);
		expect(model.problems[0]?.related.map(member => member.resource)).toEqual([pod]);
		controller.namespace = "other";
		expect(buildOverviewModel([controller, pod]).problems).toHaveLength(2);
	});
	test("a name-only reference cannot fold an absent owner into an unrelated controller", () => {
		const deployment = row("Deployment", "api");
		const pod = row("Pod", "api-0", "degraded", { ownerRef: "api" });
		expect(buildOverviewModel([deployment, pod]).problems).toHaveLength(2);
	});
	test("same-named kinds have a stable order independent of list order", () => {
		const ingress = row("Ingress", "api");
		const deployment = row("Deployment", "api");
		expect(buildOverviewModel([ingress, deployment]).problems.map(problem => problem.resource.kind)).toEqual(["Deployment", "Ingress"]);
	});
	test("folds a failing owner chain once and reports the Pod cause and its own restart count", () => {
		const deployment = row("Deployment", "api");
		const replica = row("ReplicaSet", "api-rs", "degraded", { controllerOwner: owner("Deployment", "api") });
		const pod = row("Pod", "api-pod", "degraded", { controllerOwner: owner("ReplicaSet", "api-rs"), restarts: 12 });
		const model = buildOverviewModel([pod, deployment, replica]);
		expect(model.problems).toHaveLength(1);
		expect(model.problemResources).toBe(3);
		expect(model.problems[0]?.resource).toBe(deployment);
		expect(model.problems[0]?.cause).toBe(pod);
		expect(model.problems[0]?.restarts).toBe(12);
		expect(model.problems[0]?.related.map(member => member.resource)).toEqual([pod, replica]);
	});
	test("does not merge same names across namespaces or contexts", () => {
		const rows = ["dev", "prod"].flatMap(cluster => ["a", "b"].flatMap(namespace => [
			row("Deployment", "api", "degraded", { cluster, namespace }),
			row("Pod", "api-pod", "degraded", { cluster, namespace, controllerOwner: owner("Deployment", "api") }),
		]));
		const model = buildOverviewModel(rows);
		expect(model.problems).toHaveLength(4);
		expect(model.problems.every(problem => problem.related.length === 1)).toBe(true);
	});
	test("leaves missing, ambiguous and cyclic owners visible", () => {
		const rows = [
			row("Pod", "orphan", "degraded", { ownerRef: "missing" }),
			row("Deployment", "same"), row("StatefulSet", "same"),
			row("Pod", "ambiguous", "degraded", { ownerRef: "same" }),
			row("ReplicaSet", "cycle-a", "degraded", { controllerOwner: owner("ReplicaSet", "cycle-b") }),
			row("ReplicaSet", "cycle-b", "degraded", { controllerOwner: owner("ReplicaSet", "cycle-a") }),
		];
		expect(buildOverviewModel(rows).problems).toHaveLength(rows.length);
	});
	test("walks healthy intermediates while keeping the cause at the winning severity", () => {
		const deployment = row("Deployment", "api");
		const replica = row("ReplicaSet", "api-rs", "healthy", { controllerOwner: owner("Deployment", "api") });
		const pod = row("Pod", "api-pod", "needsAttention", { controllerOwner: owner("ReplicaSet", "api-rs"), restarts: 9 });
		const problem = buildOverviewModel([deployment, replica, pod]).problems[0];
		expect(problem?.related.map(member => member.resource)).toEqual([pod]);
		expect(problem?.cause).toBe(deployment);
		expect(problem?.tone).toBe("degraded");
	});
	test("historical restarts and unknown health do not create current findings", () => {
		const model = buildOverviewModel([row("Pod", "recovered", "healthy", { restarts: 12 }), row("ConfigMap", "config", "notEvaluated"), row("Pod", "unknown", "unknown")]);
		expect(model.problems).toEqual([]);
		expect(model.namespaces[0]).toMatchObject({ total: 3, degraded: 0, attention: 0 });
	});
});
