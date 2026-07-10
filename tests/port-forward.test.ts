import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import type { PortForwardSessionSummary, ResourceSummary } from "../src/lib/types";
import {
	extractServicePortOptions,
	isPortForwardForResource,
	isReusablePortForwardSession,
	parsePortForwardForm,
	parseSavedPortForwardForm,
	portForwardLocalUrl,
	portForwardSessionToRequest,
	savedPortForwardMatchesSession,
	savedPortForwardToRequest,
	sortPortForwardSessions,
} from "../src/features/live-sessions/helpers";
import { createSavedPortForward } from "../src/lib/workspaces";

const baseSession: PortForwardSessionSummary = {
	id: "port-forward-1",
	clusterContext: "kind-dev",
	namespace: "payments",
	targetKind: "Pod",
	targetName: "api-0",
	podName: "api-0",
	remotePort: 8080,
	resolvedPodName: "api-0",
	resolvedPodPort: 8080,
	localPort: 18080,
	localAddress: "127.0.0.1",
	localUrl: "http://127.0.0.1:18080",
	status: "listening",
	startedAt: "2026-05-31T00:00:00Z",
};

const baseResource: ResourceSummary = {
	kind: "Pod",
	cluster: "kind-dev",
	name: "api-0",
	namespace: "payments",
	age: "1m",
};

describe("port-forward helpers", () => {
	test("validates user-entered ports before command invocation", () => {
		expect(parsePortForwardForm({ remotePort: "8080", localPort: "" })).toEqual({
			remotePort: 8080,
		});
		expect(parsePortForwardForm({ remotePort: "8080", localPort: "18080" })).toEqual({
			remotePort: 8080,
			localPort: 18080,
		});
		expect(parsePortForwardForm({ remotePort: "", localPort: "" })).toBe(
			"Remote port is required",
		);
		expect(
			parsePortForwardForm(
				{ remotePort: "", localPort: "" },
				{ remotePortLabel: "Service port" },
			),
		).toBe("Service port is required");
		expect(parsePortForwardForm({ remotePort: "70000", localPort: "" })).toBe(
			"Remote port must be between 1 and 65535",
		);
		expect(parsePortForwardForm({ remotePort: "8080", localPort: "80" })).toBe(
			"Local port must be 1024 or higher",
		);
	});

	test("extracts selectable TCP Service ports from resource YAML", () => {
		const yaml = `apiVersion: v1
kind: Service
metadata:
  name: api
spec:
  type: ClusterIP
  ports:
  - name: http
    port: 8080
    protocol: TCP
    targetPort: 8080
  - name: metrics
    port: 9090
    targetPort: metrics
  - name: dns
    port: 53
    protocol: UDP
`;

		expect(extractServicePortOptions(yaml)).toEqual([
			{ name: "http", port: 8080, protocol: "TCP", targetPort: "8080" },
			{ name: "metrics", port: 9090, targetPort: "metrics" },
		]);
	});

	test("matches active sessions to the selected target resource", () => {
		expect(isPortForwardForResource(baseSession, baseResource)).toBe(true);
		expect(
			isPortForwardForResource(
				{ ...baseSession, targetName: "worker-0", podName: "worker-0" },
				baseResource,
			),
		).toBe(false);
		expect(
			isPortForwardForResource(baseSession, {
				...baseResource,
				kind: "Deployment",
			}),
		).toBe(false);
		expect(
			isPortForwardForResource(
				{
					...baseSession,
					targetKind: "Service",
					targetName: "api",
					podName: "api-7c9f",
					resolvedPodName: "api-7c9f",
				},
				{
					...baseResource,
					kind: "Service",
					name: "api",
				},
			),
		).toBe(true);
	});

	test("formats and sorts active sessions for stable UI", () => {
		const sessions = [
			{
				...baseSession,
				id: "b",
				targetName: "worker-0",
				podName: "worker-0",
				resolvedPodName: "worker-0",
				localPort: 18081,
			},
			{ ...baseSession, id: "a" },
		];

		expect(portForwardLocalUrl({ ...baseSession, localUrl: "" })).toBe(
			"http://127.0.0.1:18080",
		);
		expect(sortPortForwardSessions(sessions).map((session) => session.id)).toEqual([
			"a",
			"b",
		]);
	});

	test("builds Service requests from saved presets and matches active sessions", () => {
		const saved = createSavedPortForward(
			{
				clusterContext: "kind-dev",
				namespace: "payments",
				serviceName: "api",
				servicePort: 8080,
			},
			"2026-05-31T00:00:00Z",
		);
		const serviceSession: PortForwardSessionSummary = {
			...baseSession,
			targetKind: "Service",
			targetName: "api",
			podName: "api-7c9f",
			resolvedPodName: "api-7c9f",
		};

		expect(savedPortForwardToRequest(saved)).toEqual({
			clusterContext: "kind-dev",
			namespace: "payments",
			targetKind: "Service",
			targetName: "api",
			remotePort: 8080,
			localPort: undefined,
		});
		expect(savedPortForwardMatchesSession(saved, serviceSession)).toBe(true);
		expect(portForwardSessionToRequest(serviceSession)).toEqual({
			clusterContext: "kind-dev",
			namespace: "payments",
			targetKind: "Service",
			targetName: "api",
			remotePort: 8080,
			localPort: 18080,
		});
		expect(isReusablePortForwardSession(serviceSession)).toBe(true);
		expect(
			isReusablePortForwardSession({ ...serviceSession, status: "error" }),
		).toBe(false);
		expect(
			savedPortForwardMatchesSession(
				{ ...saved, localPort: 19090 },
				serviceSession,
			),
		).toBe(false);
		expect(
			savedPortForwardToRequest({ ...saved, localPort: 19090 }).localPort,
		).toBe(19090);
	});

	test("validates saved Service port-forward presets", () => {
		expect(
			parseSavedPortForwardForm({
				clusterContext: "kind-dev",
				namespace: "payments",
				serviceName: "api",
				servicePort: "8080",
				localPort: "",
				label: "API",
			}),
		).toEqual({
			clusterContext: "kind-dev",
			namespace: "payments",
			serviceName: "api",
			servicePort: 8080,
			localPort: undefined,
			label: "API",
		});
		expect(
			parseSavedPortForwardForm({
				clusterContext: "",
				namespace: "payments",
				serviceName: "api",
				servicePort: "8080",
				localPort: "",
				label: "",
			}),
		).toBe("Cluster context is required");
		expect(
			parseSavedPortForwardForm({
				clusterContext: "kind-dev",
				namespace: "payments",
				serviceName: "api",
				servicePort: "8080",
				localPort: "80",
				label: "",
			}),
		).toBe("Local port must be 1024 or higher");
	});

	test("workspace manager renders active session controls and saved errors", () => {
		const svelteSurfaceSource = readFileSync(
			"src/app/svelte/AppSurfaces.svelte",
			"utf8",
		);
		const svelteLiveSurfaceSource = [
			svelteSurfaceSource,
			readFileSync("src/app/svelte/LiveSessionsSurface.svelte", "utf8"),
		].join("\n");
		const svelteAppSource = readFileSync(
			"src/app/svelte/App.svelte",
			"utf8",
		);
		const svelteShellSource = readFileSync(
			"src/app/svelte/WorkspaceShell.svelte",
			"utf8",
		);
		const svelteChromeSource = readFileSync(
			"src/app/svelte/ActiveLiveSessionsButton.svelte",
			"utf8",
		);

		expect(svelteLiveSurfaceSource).toContain("portForward.lastError");
		expect(svelteLiveSurfaceSource).toContain("stopPortForwardSession(session.id)");
		expect(svelteLiveSurfaceSource).toContain("reconnectPortForward(session)");
		expect(svelteLiveSurfaceSource).toContain("portForwardSessionResolution(session)");
		expect(svelteLiveSurfaceSource).toContain("reconnectPortForward(session)");
		expect(svelteLiveSurfaceSource).toContain("visiblePortForwardSessions");
		expect(svelteLiveSurfaceSource).toContain("visibleExecSessions");
		expect(svelteLiveSurfaceSource).toContain("workspaceContexts.includes(session.clusterContext)");
		expect(svelteLiveSurfaceSource).toContain("showKubeconfigSourceLabels && session.kubeconfigSourceLabel");
		expect(svelteLiveSurfaceSource).toContain("podExecCommandText(session.command)");
		expect(svelteLiveSurfaceSource).not.toContain(
			"Exec sessions\", sessionsQuery.data?.execSessions.length",
		);
		expect(svelteSurfaceSource).toContain(
			"placeholderData: (previousData) => previousData",
		);
		expect(svelteSurfaceSource).toContain("queryKey: queryKeys.podExecSessions()");
		expect(svelteSurfaceSource).toContain("portForwardQueryOptions(client");
		expect(svelteSurfaceSource).toContain("portForwardSessionsForWorkspace(");
		expect(svelteLiveSurfaceSource).toContain("query={liveSessionsQuery}");
		expect(svelteSurfaceSource).not.toContain('"svelte-live-sessions-surface"');
		expect(svelteLiveSurfaceSource).toContain("Auto-start saved");
		expect(svelteSurfaceSource).toContain("startAllSavedPortForwards");
		expect(svelteShellSource).toContain("ActiveLiveSessionsButton");
		expect(svelteShellSource).toContain("onOpenManager={openPortForwards}");
		expect(svelteShellSource).toContain("Live sessions updated");
		expect(svelteAppSource).toContain("stopLiveSessionsOutsideScope");
		expect(svelteAppSource).toContain(
			"$settingsStore.keepLiveSessionsOnWorkspaceSwitch",
		);
		expect(svelteAppSource).toContain("workspaceScopeContexts(workspace.scope)");
		expect(svelteAppSource).toContain("invalidatePortForwardQueries(");
		expect(svelteAppSource).toContain("queryKeys.podExecSessions()");
		expect(svelteAppSource).toContain(
			"Stopped $" + "{result.stoppedPortForwards} port",
		);
		expect(svelteChromeSource).toContain("portForwardQueryOptions(client)");
		expect(svelteChromeSource).toContain("listPodExecSessions(client)");
		expect(svelteChromeSource).toContain("queryKeys.podExecSessions()");
	});

	test("Service detail forwarding offers a port picker when ports are known", () => {
		const source = readFileSync(
			"src/features/resource-detail/PortForwardTab.svelte",
			"utf8",
		);

		expect(source).toContain("extractServicePortOptions");
		expect(source).toContain("<Select");
		expect(source).toContain("<SelectGroup>");
		expect(source).toContain("Save preset");
		expect(source).toContain("Preset already saved");
		expect(source).toContain("workspaceScopeContexts(activeWorkspace.scope)");
		expect(source).toContain("Workspace context must include this Service");
		expect(source).toContain("workspaceStore.saveSavedPortForward");
		expect(source).toContain("copySessionUrl");
		expect(source).toContain("navigator.clipboard?.writeText");
		expect(source).toContain("portForwardLocalUrl(session)");
		expect(source).toContain("$settingsStore.showKubeconfigSourceLabels");
		expect(source).toContain("session.kubeconfigSourceLabel");
		expect(source).toContain("sessionTitle(session)");
		expect(source).toContain("sessionResolution(session)");
		expect(source).toContain("Resolved to Pod/");
		expect(source).toContain('localPort = "";');
	});
});
