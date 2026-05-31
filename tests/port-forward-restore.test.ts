import { describe, expect, test } from "bun:test";
import {
	savedPortForwardStartFailureMessage,
	shouldAutoStartSavedPortForwards,
	shouldShowSavedPortForwardRestorePrompt,
} from "../src/features/live-sessions/restore";
import { createSavedPortForward, createWorkspaceRecord } from "../src/lib/workspaces";

describe("saved port-forward restore behavior", () => {
	test("prompts when saved forwards exist and auto-start is off", () => {
		const workspace = createWorkspaceRecord({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["payments"],
		});
		workspace.portForwards = [
			createSavedPortForward({
				clusterContext: "kind-dev",
				namespace: "payments",
				serviceName: "api",
				servicePort: 8080,
			}),
		];

		expect(
			shouldShowSavedPortForwardRestorePrompt({
				workspace,
				autoStart: false,
				dismissedWorkspaceId: null,
			}),
		).toBe(true);
		expect(
			shouldShowSavedPortForwardRestorePrompt({
				workspace,
				autoStart: false,
				dismissedWorkspaceId: workspace.id,
			}),
		).toBe(false);
		expect(
			shouldShowSavedPortForwardRestorePrompt({
				workspace,
				autoStart: true,
				dismissedWorkspaceId: null,
			}),
		).toBe(false);
	});

	test("auto-starts each workspace once when the global setting is enabled", () => {
		const workspace = createWorkspaceRecord({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["payments"],
		});
		workspace.portForwards = [
			createSavedPortForward({
				clusterContext: "kind-dev",
				namespace: "payments",
				serviceName: "api",
				servicePort: 8080,
			}),
		];
		const startedWorkspaceIds = new Set<string>();

		expect(
			shouldAutoStartSavedPortForwards({
				workspace,
				autoStart: true,
				startedWorkspaceIds,
			}),
		).toBe(true);
		startedWorkspaceIds.add(workspace.id);
		expect(
			shouldAutoStartSavedPortForwards({
				workspace,
				autoStart: true,
				startedWorkspaceIds,
			}),
		).toBe(false);
	});

	test("reports saved forward start failures without dismissing the prompt", () => {
		expect(
			savedPortForwardStartFailureMessage([
				{ ok: true },
				{ ok: false },
				{ ok: false },
			]),
		).toBe("2 saved forwards failed to start. Review port forwards for details.");
		expect(savedPortForwardStartFailureMessage([{ ok: true }])).toBeNull();
	});
});
