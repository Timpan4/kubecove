// Same 10,000-row workload as benchmarks/resource-table.bench.ts.
// Usage: bun e2e/parity/visual/export-watch-burst.ts <browser-fixture.json> <output.json>
import { readFile, writeFile } from "node:fs/promises";
import type { DiscoveredResourceKind, HealthAssessment, ResourceMetricSummary, ResourceSummary } from "../../../src/lib/types";

const kinds = [
	"Deployment",
	"Pod",
	"Service",
	"ConfigMap",
	"Ingress",
	"Widget",
];

const widgetKind: DiscoveredResourceKind = {
	group: "operator.example.com",
	version: "v1beta1",
	apiVersion: "operator.example.com/v1beta1",
	kind: "Widget",
	plural: "widgets",
	namespaced: true,
};

function healthAssessment(state: "degraded" | "healthy"): HealthAssessment {
	return {
		state,
		completeness: "complete",
		winningSources: ["kubernetes"],
		reasons: [state === "healthy" ? "Ready=True" : "Ready=False"],
		evidence: [
			{
				source: "kubernetes",
				raw: { type: "Ready", status: state === "healthy" ? "True" : "False" },
				state,
				current: true,
				reason: state === "healthy" ? "Ready=True" : "Ready=False",
			},
		],
	};
}

function metricSummary(
	kind: string,
	name: string,
	namespace: string,
	index: number,
): ResourceMetricSummary {
	return {
		kind,
		cluster: "prod",
		name,
		namespace,
		cpuMillicores: (index % 64) * 10,
		memoryBytes: (index % 512) * 1024 * 1024,
		sourcePods: [],
	};
}

function resourceSummary(index: number): ResourceSummary {
	const kind = kinds[index % kinds.length] ?? "Pod";
	const namespace = `namespace-${index % 100}`;
	const app = `checkout-${index % 250}`;
	const healthState = index % 17 === 0 ? "degraded" : "healthy";
	const name = `${app}-${kind.toLowerCase()}-${index}`;
	return {
		cluster: "prod",
		apiVersion:
			kind === "Pod" || kind === "Service" || kind === "ConfigMap"
				? "v1"
				: kind === "Widget"
					? widgetKind.apiVersion
					: "apps/v1",
		kind,
		name,
		namespace,
		age: `${index % 60}m`,
		health: healthState,
		createdAt: `2026-01-${String((index % 28) + 1).padStart(2, "0")}T00:00:00Z`,
		status: index % 17 === 0 ? "CrashLoopBackOff" : "Running",
		healthAssessment: healthAssessment(healthState),
		ready: index % 17 === 0 ? "False" : "True",
		restarts: index % 17 === 0 ? 5 : index % 3,
		ownerRef: kind === "Pod" ? `${app}-deployment` : undefined,
		argoApp: index % 3 === 0 ? app : undefined,
		gitOpsOwner:
			index % 3 === 0
				? {
						provider: "argo",
						kind: "Application",
						name: app,
						namespace: "argocd",
						confidence: "metadata",
					}
				: undefined,
		metrics: metricSummary(kind, name, namespace, index),
	};
}

const rows = Array.from({ length: 10_000 }, (_, index) =>
	resourceSummary(index),
);

const [input, output] = process.argv.slice(2);
if (!input || !output) throw new Error("Expected input and output fixture paths");
const fixture = JSON.parse(await readFile(input, "utf8"));
fixture.contexts = [{ name: "prod", isCurrent: true }];
fixture.kinds.push(widgetKind);
fixture.resources = rows.map((row) => ({
    ...row,
    gitOpsOwner: row.gitOpsOwner ? { provenance: "benchmark", partial: true, ...row.gitOpsOwner } : undefined,
}));
await writeFile(output, JSON.stringify(fixture));
