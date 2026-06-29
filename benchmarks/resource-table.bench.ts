import { bench, describe } from "vitest";
import {
	buildResourceTableModel,
	type ResourceTableState,
} from "@/features/resources/resourceBrowserModel";
import {
	buildResourceSearchIndex,
	filterResourceSearchIndex,
} from "@/features/resources/helpers";
import type { ResourceSummary } from "@/lib/types";

const kinds = ["Deployment", "Pod", "Service", "ConfigMap", "Ingress"];

function resourceSummary(index: number): ResourceSummary {
	const kind = kinds[index % kinds.length] ?? "Pod";
	const namespace = `namespace-${index % 100}`;
	const app = `checkout-${index % 250}`;
	return {
		cluster: "prod",
		apiVersion:
			kind === "Pod" || kind === "Service" || kind === "ConfigMap"
				? "v1"
				: "apps/v1",
		kind,
		name: `${app}-${kind.toLowerCase()}-${index}`,
		namespace,
		age: `${index % 60}m`,
		createdAt: `2026-01-${String((index % 28) + 1).padStart(2, "0")}T00:00:00Z`,
		status: index % 17 === 0 ? "CrashLoopBackOff" : "Running",
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
					}
				: undefined,
		metrics: {
			cpuMillicores: (index % 64) * 10,
			memoryBytes: (index % 512) * 1024 * 1024,
		},
	};
}

const rows = Array.from({ length: 10_000 }, (_, index) =>
	resourceSummary(index),
);
const searchIndex = buildResourceSearchIndex(rows);
const gitOpsFilter = "argo:Application:argocd:checkout-42";
const state: ResourceTableState = {
	search: "checkout",
	gitOpsFilter: "",
	healthFilter: "unhealthy",
	sort: { id: "memory", desc: true },
	pageIndex: 0,
	collapsedGroups: new Set(),
	selectedResource: rows[42],
};

const gitOpsState: ResourceTableState = {
	...state,
	search: "checkout",
	gitOpsFilter,
	healthFilter: "all",
};

describe("resource table model (10k rows)", () => {
	bench("buildResourceSearchIndex", () => {
		buildResourceSearchIndex(rows);
	});

	bench("filterResourceSearchIndex (search + gitops owner)", () => {
		filterResourceSearchIndex(searchIndex, "checkout-42", gitOpsFilter);
	});

	bench("buildResourceTableModel (search + health + sort + groups)", () => {
		buildResourceTableModel(rows, state);
	});

	bench("buildResourceTableModel (gitops owner filtered)", () => {
		buildResourceTableModel(rows, gitOpsState);
	});
});
