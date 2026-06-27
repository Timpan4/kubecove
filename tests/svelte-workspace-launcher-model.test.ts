import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createWorkspaceRecord } from "../src/lib/workspace-model";
import {
	buildWorkspaceInput,
	pickEffectiveContext,
	uniqueWorkspaceContexts,
} from "../src/features/workspaces/workspaceLauncherModel";
import type { ClusterContext } from "../src/lib/types";

const contexts: ClusterContext[] = [
	{ name: "kind-dev", isCurrent: false },
	{ name: "kind-prod", isCurrent: true },
];

describe("svelte workspace launcher model", () => {
	test("picks selected, current, then first context", () => {
		expect(pickEffectiveContext("kind-dev", contexts)).toBe("kind-dev");
		expect(pickEffectiveContext("", contexts)).toBe("kind-prod");
		expect(
			pickEffectiveContext("", [{ name: "kind-only", isCurrent: false }]),
		).toBe("kind-only");
	});

	test("creates React-compatible context group input", () => {
		const input = buildWorkspaceInput({
			name: "Ops",
			effectiveContext: "kind-prod",
			selectedClusterContexts: uniqueWorkspaceContexts("kind-prod", [
				"kind-dev",
				"kind-prod",
			]),
			selectedNamespaces: ["argocd", "default"],
		});

		expect(input).toMatchObject({
			name: "Ops",
			clusterContext: "kind-prod",
			clusterContexts: ["kind-prod", "kind-dev"],
			clusterGroupName: "Ops group",
			namespaces: ["argocd", "default"],
		});
	});

	test("edit input preserves existing kinds and shortcut preferences", () => {
		const workspace = createWorkspaceRecord({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["default"],
			shortcutPreferences: { includeArgo: false },
		});

		const input = buildWorkspaceInput({
			name: "",
			effectiveContext: "kind-prod",
			selectedClusterContexts: ["kind-prod"],
			selectedNamespaces: [],
			editingWorkspace: workspace,
		});

		expect(input.name).toBe("kind-prod");
		expect(input.kinds).toBe(workspace.scope.kinds);
		expect(input.shortcutPreferences).toBe(workspace.scope.shortcutPreferences);
	});

	test("Svelte launcher keeps backend context and namespace state in svelte-query", () => {
		const source = readFileSync(
			"src/features/workspaces/WorkspaceLauncher.svelte",
			"utf8",
		);

		expect(source).toContain('import { createQuery } from "@tanstack/svelte-query";');
		expect(source).toContain("queryKeys.kubeContexts(kubeconfigSourceKey)");
		expect(source).toContain("queryKeys.namespaces(effectiveContext, kubeconfigSourceKey)");
		expect(source).not.toContain("async function loadContexts");
		expect(source).not.toContain("async function loadNamespaces");
	});
});
