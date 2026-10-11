import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	argoReconciliationResources,
	argoResourceCounts,
	argoResourceIdentityKey,
	filterWorkspaceResourcesByArgo,
	preserveArgoResourceSelection,
} from "@/features/gitops/argo-workspace-model";
import type { ArgoManagedResource } from "@/lib/types";
import { workspaceRows } from "./workspace-benchmark-fixtures";

const managedResources: ArgoManagedResource[] = Array.from(
	{ length: 2_500 },
	(_, index) => {
		const resource = workspaceRows[index * 4];
		if (!resource)
			throw new Error(`Missing workspace fixture row ${index * 4}`);
		return {
			group: resource.apiVersion === "v1" && index % 2 === 0 ? "" : undefined,
			apiVersion: resource.apiVersion,
			kind: resource.kind,
			namespace: resource.namespace,
			name: resource.name,
			status: index % 5 === 0 ? "OutOfSync" : "Synced",
			health:
				index % 17 === 0
					? "Degraded"
					: index % 3 === 0
						? "Progressing"
						: "Healthy",
			requiresPruning: index % 19 === 0,
		};
	},
);
const duplicateResource = managedResources[42];
if (!duplicateResource)
	throw new Error("Missing managed resource fixture row 42");
managedResources.push(
	{ ...duplicateResource },
	{ kind: "Deployment", namespace: "namespace-0", status: "Synced" },
);
const selected = managedResources[42];
if (!selected)
	throw new Error("Missing managed resource selection fixture row 42");
const allManaged = filterWorkspaceResourcesByArgo(
	workspaceRows,
	managedResources,
	"allManaged",
);

assert.equal(workspaceRows.length, 10_000);
assert.equal(managedResources.length, 2_502);
assert(allManaged.length > 0);
assert.equal(allManaged.length, 2_500);
assert.equal(
	argoResourceIdentityKey(managedResources[2_500]),
	argoResourceIdentityKey(duplicateResource),
);
assert.equal(
	preserveArgoResourceSelection({ ...selected }, managedResources),
	duplicateResource,
);
assert.equal(
	preserveArgoResourceSelection(
		{ ...selected, name: "missing" },
		managedResources,
	),
	null,
);
assert.equal(
	filterWorkspaceResourcesByArgo(workspaceRows, [], "none"),
	workspaceRows,
);
assert(
	argoReconciliationResources(managedResources).some(
		(resource) => resource.requiresPruning,
	),
);

describe("Argo managed resources F47", () => {
	bench(
		"F47 managed counts, reconciliation rows, workspace matching, and selection reconciliation",
		() => {
			argoResourceCounts(managedResources);
			argoReconciliationResources(managedResources);
			filterWorkspaceResourcesByArgo(
				workspaceRows,
				managedResources,
				"needsSync",
			);
			filterWorkspaceResourcesByArgo(
				workspaceRows,
				managedResources,
				"degraded",
			);
			filterWorkspaceResourcesByArgo(workspaceRows, managedResources, "prune");
			preserveArgoResourceSelection(selected, managedResources);
		},
	);
});
