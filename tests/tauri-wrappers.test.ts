import { describe, expect, test } from "bun:test";
import {
	createMockTauriClient,
	createMockChannel,
	addKubeconfigPaths,
	clearBackendDiagnostics,
	detectFlux,
	listArgoAppProjects,
	listArgoApplicationSets,
	listArgoApplications,
	listIncidentCockpit,
	getBackendDiagnostics,
	getAppUsageMetrics,
	getFluxResourceDetails,
	getHelmReleaseDetails,
	getKubeconfigSources,
	getHelmReleaseReconciliation,
	listResourceMetrics,
	listResourceScope,
	listRbacInspection,
	getResourceYaml,
	isTauriRuntime,
	isAppError,
	listPortForwards,
	listPodExecSessions,
	listHelmReleases,
	listFluxResources,
	listPresentCustomResourceKinds,
	listResourceTopology,
	listKubeContexts,
	listNamespaces,
	resizePodExecTerminal,
	removeKubeconfigPath,
	reorderKubeconfigPaths,
	setKubeconfigEnvVar,
	setBackendDiagnosticsEnabled,
	setShowKubeconfigSourceLabels,
	shouldUseBrowserDevMocks,
	startPodLogStream,
	startPodExecSession,
	startPodPortForward,
	startPortForward,
	stopLiveSessionsOutsideScope,
	stopPodExecSession,
	stopPodPortForward,
	writePodExecStdin,
} from "../src/lib/tauri";
import { createDevMockTauriClient } from "../src/lib/tauri-dev-mocks";
import { queryKeys } from "../src/lib/queryKeys";
import { kubeconfigSourceKey } from "../src/lib/settings";
import type {
	AppUsageMetrics,
	ClusterContext,
	FluxDetectionSummary,
	FluxResourceDetails,
	FluxResourceKind,
	FluxResourceSummary,
	HelmReleaseDetails,
	HelmReleaseReconciliation,
	HelmReleaseSummary,
	NamespaceSummary,
	PodExecSessionSummary,
	PortForwardSessionSummary,
	RbacInspectionSummary,
	ResourceMetricsSummary,
	ResourceTopology,
	KubeconfigSourcesSummary,
	LiveSessionCleanupResult,
	BackendDiagnosticEvent,
} from "../src/lib/types";

describe("createMockTauriClient", () => {
	test("returns mock response for known command", async () => {
		const mockContexts: ClusterContext[] = [
			{ name: "minikube", isCurrent: true },
			{ name: "docker-desktop", isCurrent: false },
		];
		const client = createMockTauriClient({
			list_kube_contexts: mockContexts,
		});

		expect(await listKubeContexts(client)).toEqual(mockContexts);
	});

	test("passes args to handler mock responses", async () => {
		const client = createMockTauriClient({
			list_namespaces: (args) => [
				{
					name: args?.clusterContext === "prod" ? "payments" : "default",
					age: "1d",
				},
			],
		});

		expect(await listNamespaces(client, "prod")).toEqual([
			{ name: "payments", age: "1d" },
		]);
	});

	test("throws for unknown command", async () => {
		const client = createMockTauriClient({});

		await expect(listKubeContexts(client)).rejects.toThrow(
			"No mock response for command: list_kube_contexts",
		);
	});

	test("detects browser dev mock runtime", () => {
		expect(shouldUseBrowserDevMocks({ DEV: true }, {})).toBe(true);
		expect(shouldUseBrowserDevMocks({ DEV: false }, {})).toBe(false);
		expect(
			shouldUseBrowserDevMocks(
				{ DEV: true },
				{ __TAURI_INTERNALS__: { invoke: () => undefined } },
			),
		).toBe(false);
		expect(isTauriRuntime({ isTauri: true })).toBe(true);
	});

	test("mock channel stops forwarding after cleanup", () => {
		const messages: string[] = [];
		const channel = createMockChannel<string>((message) => messages.push(message)) as ReturnType<
			typeof createMockChannel<string>
		> & { cleanupCallback: () => void };

		channel.onmessage("connected");
		channel.cleanupCallback();
		channel.onmessage("ignored");

		expect(messages).toEqual(["connected"]);
		expect(channel.toJSON()).toStartWith("__MOCK_CHANNEL__:");
	});

	test("browser dev topology returns mode-specific canonical graph data", async () => {
		const client = createDevMockTauriClient();
		const ownership = await listResourceTopology(client, "mock-dev", ["payments"], "ownership");
		const network = await listResourceTopology(client, "mock-dev", ["payments"], "networkFlow");

		expect(ownership.nodes.map((node) => node.id)).toContain(
			"mock-dev:apps/v1:Deployment:payments:payments-api",
		);
		expect(ownership.nodes.map((node) => node.kind)).toContain("ReplicaSet");
		expect(ownership.nodes.map((node) => node.kind)).toContain("ConfigMap");
		expect(ownership.nodes.map((node) => node.kind)).toContain("Secret");
		expect(ownership.nodes.map((node) => node.kind)).toContain("Service");
		expect(ownership.edges.every((edge) => edge.relation !== "routesTo")).toBe(true);
		expect(network.edges.map((edge) => edge.relation)).toContain("routesTo");
		expect(network.edges.map((edge) => edge.relation)).toContain("targets");
		expect(network.nodes.some((node) => node.portHints?.length)).toBe(true);
	});

	test("browser dev ownership topology matches real support-resource shape", async () => {
		const client = createDevMockTauriClient();
		const ownership = await listResourceTopology(client, "admin@solid-k8s", ["argocd", "monitoring", "todo", "jobs-lab"], "ownership");

		expect(ownership.nodes.map((node) => node.id)).toContain(
			"admin@solid-k8s:v1:ConfigMap:argocd:argocd-cm",
		);
		expect(ownership.nodes.map((node) => node.id)).toContain(
			"admin@solid-k8s:v1:Secret:argocd:argocd-secret",
		);
		expect(ownership.nodes.map((node) => node.id)).toContain(
			"admin@solid-k8s:v1:Service:monitoring:grafana",
		);
		expect(ownership.nodes.map((node) => node.id)).toContain(
			"admin@solid-k8s:v1:ConfigMap:todo:todo-web-content",
		);
		expect(ownership.nodes.map((node) => node.id)).toContain(
			"admin@solid-k8s:v1:Secret:todo:todo-web-session",
		);
		expect(ownership.nodes.map((node) => node.id)).toContain(
			"admin@solid-k8s:v1:Service:todo:todo-web",
		);
		expect(ownership.nodes.map((node) => node.name)).not.toContain("kube-root-ca.crt");
		expect(ownership.edges.every((edge) => edge.relation === "owns")).toBe(true);
	});

	test("browser dev mock keeps support resources attached to workload owners", async () => {
		const client = createDevMockTauriClient();
		const ownership = await listResourceTopology(client, "mock-dev", ["payments"], "ownership");
		const deploymentId = "mock-dev:apps/v1:Deployment:payments:payments-api";

		for (const supportId of [
			"mock-dev:v1:ConfigMap:payments:payments-api-config",
			"mock-dev:v1:Secret:payments:payments-api-secrets",
			"mock-dev:v1:Service:payments:payments-api",
			"mock-dev:networking.k8s.io/v1:Ingress:payments:payments-api",
		]) {
			expect(ownership.edges).toContainEqual({
				id: `${deploymentId}->${supportId}`,
				source: deploymentId,
				target: supportId,
				relation: "owns",
			});
		}
		expect(ownership.warnings.join(" ")).not.toContain("static owner");
	});

	test("browser dev mock exposes richer network, compare, and live-session surfaces", async () => {
		const client = createDevMockTauriClient();
		const network = await listResourceTopology(client, "mock-dev", [], "networkFlow");
		const mockDeployments = await listResourceScope(client, "mock-dev", [{ kind: "Deployment" }]);
		const dockerDeployments = await listResourceScope(client, "docker-desktop", [{ kind: "Deployment" }]);
		const portForwards = await listPortForwards(client);
		const execSessions = await listPodExecSessions(client);

		expect(network.edges.filter((edge) => edge.relation === "routesTo").length).toBeGreaterThanOrEqual(4);
		expect(network.nodes.filter((node) => node.kind === "EndpointSlice").length).toBeGreaterThanOrEqual(4);
		expect(mockDeployments.map((row) => row.name)).toContain("payments-api");
		expect(dockerDeployments.map((row) => row.name)).toEqual([
			"registry",
			"ingress-nginx-controller",
		]);
		expect(portForwards.map((session) => session.localUrl)).toContain("http://127.0.0.1:13000");
		expect(execSessions.map((session) => session.command.join(" "))).toContain("/bin/sh");
	});

	test("browser dev mock provides deeper GitOps, Helm, RBAC, and incident data", async () => {
		const client = createDevMockTauriClient();
		const fluxDetection = await detectFlux(client, "mock-dev");
		const fluxResources = (await Promise.all(
			fluxDetection.kinds.map((kind) => listFluxResources(client, "mock-dev", kind)),
		)).flat();
		const incidents = await listIncidentCockpit(client, "docker-desktop", []);
		const rbac = await listRbacInspection(client, "mock-dev", ["payments", "monitoring", "argocd"]);

		expect(await listArgoApplications(client, "mock-dev")).toHaveLength(2);
		expect(await listArgoApplicationSets(client, "mock-dev")).toHaveLength(2);
		expect(await listArgoAppProjects(client, "mock-dev")).toHaveLength(2);
		expect(fluxResources.map((row) => row.resourceKind.kind)).toEqual([
			"Kustomization",
			"HelmRelease",
		]);
		expect(await listHelmReleases(client, "mock-dev")).toHaveLength(2);
		expect(rbac.serviceAccounts).toHaveLength(3);
		expect(rbac.roles.length).toBeGreaterThan(0);
		expect(rbac.clusterRoleBindings.length).toBeGreaterThan(0);
		expect(incidents.items.map((item) => item.resource.name)).toEqual([
			"ingress-nginx-controller",
			"ingress-nginx-controller-7bdbf967f9-dk4nc",
		]);
	});

	test("passes Pod exec requests through typed wrappers", async () => {
		const session: PodExecSessionSummary = {
			id: "pod-exec-1",
			clusterContext: "kind-dev",
			namespace: "payments",
			podName: "api-0",
			container: "api",
			command: ["/bin/sh"],
			stdin: true,
			tty: true,
			terminalCols: 100,
			terminalRows: 32,
			status: "running",
			startedAt: "2026-06-01T10:00:00Z",
		};
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				if (cmd === "start_pod_exec_session") return session as T;
				if (cmd === "list_pod_exec_sessions") return [session] as T;
				return true as T;
			},
		};
		const channel = {} as never;
		const request = {
			clusterContext: "kind-dev",
			namespace: "payments",
			podName: "api-0",
			container: "api",
			command: ["/bin/sh"],
			stdin: true,
			tty: true,
			terminalSize: { cols: 100, rows: 32 },
			confirmation: {
				acknowledged: true,
				target: "kind-dev/payments/Pod/api-0",
				command: "/bin/sh",
			},
		};

		expect(await startPodExecSession(client, request, channel)).toEqual(session);
		expect(await writePodExecStdin(client, session.id, "date\n")).toBe(true);
		expect(
			await resizePodExecTerminal(client, session.id, { cols: 120, rows: 40 }),
		).toBe(true);
		expect(await listPodExecSessions(client)).toEqual([session]);
		expect(await stopPodExecSession(client, session.id)).toBe(true);
		expect(calls).toEqual([
			{
				cmd: "start_pod_exec_session",
				args: { request, channel },
			},
			{
				cmd: "write_pod_exec_stdin",
				args: { sessionId: session.id, data: "date\n" },
			},
			{
				cmd: "resize_pod_exec_terminal",
				args: { sessionId: session.id, size: { cols: 120, rows: 40 } },
			},
			{
				cmd: "list_pod_exec_sessions",
				args: undefined,
			},
			{
				cmd: "stop_pod_exec_session",
				args: { sessionId: session.id },
			},
		]);
	});

	test("passes live-session cleanup scope through typed wrapper", async () => {
		const result: LiveSessionCleanupResult = {
			stoppedPortForwardIds: ["port-forward-1"],
			stoppedPodExecIds: ["pod-exec-1"],
			stoppedPortForwards: 1,
			stoppedPodExecSessions: 1,
		};
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				return result as T;
			},
		};
		const request = {
			allowedClusterContexts: ["kind-dev", "staging"],
			kubeconfigSourceKey: "kubeconfigSource=abc123",
		};

		expect(await stopLiveSessionsOutsideScope(client, request)).toEqual(result);
		expect(calls).toEqual([
			{
				cmd: "stop_live_sessions_outside_scope",
				args: { request },
			},
		]);
	});
});

describe("typed Tauri wrappers", () => {
	test("passes kubeconfig env var through cluster wrappers", async () => {
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				return [] as T;
			},
		};

		await listKubeContexts(client, "KUBECOVE_CONFIG");
		await listNamespaces(client, "kind-dev", "KUBECOVE_CONFIG");

		expect(calls).toEqual([
			{
				cmd: "list_kube_contexts",
				args: { kubeconfigEnvVar: "KUBECOVE_CONFIG" },
			},
			{
				cmd: "list_namespaces",
				args: {
					clusterContext: "kind-dev",
					kubeconfigEnvVar: "KUBECOVE_CONFIG",
				},
			},
		]);
	});

	test("omits backend source keys from kubeconfig env var args", async () => {
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				return [] as T;
			},
		};

		await listKubeContexts(client, "kubeconfigSource=abc123");
		await listNamespaces(client, "kind-dev", "kubeconfigSource=abc123");

		expect(calls).toEqual([
			{
				cmd: "list_kube_contexts",
				args: {},
			},
			{
				cmd: "list_namespaces",
				args: {
					clusterContext: "kind-dev",
				},
			},
		]);
	});

	test("query keys include kubeconfig source", () => {
		expect(kubeconfigSourceKey("")).toBe("kubeconfigEnv=KUBECONFIG");
		expect(kubeconfigSourceKey("kubeconfigSource=abc123")).toBe(
			"kubeconfigSource=abc123",
		);
		expect(queryKeys.kubeContexts("KUBECOVE_CONFIG")).toEqual([
			"kube-contexts",
			"kubeconfigEnv=KUBECOVE_CONFIG",
		]);
		expect(queryKeys.kubeContexts("kubeconfigSource=abc123")).toEqual([
			"kube-contexts",
			"kubeconfigSource=abc123",
		]);
		expect(queryKeys.namespaces("kind-dev", "KUBECOVE_CONFIG")).toEqual([
			"kube-namespaces",
			"kubeconfigEnv=KUBECOVE_CONFIG",
			"kind-dev",
		]);
		expect(queryKeys.appUsageMetrics()).toEqual(["app-usage-metrics"]);
	});

	test("passes kubeconfig source settings through typed wrappers", async () => {
		const sources: KubeconfigSourcesSummary = {
			kubeconfigEnvVar: "KUBECOVE_KUBECONFIG",
			paths: [{ path: "/tmp/a" }, { path: "/tmp/b" }],
			sourceKey: "kubeconfigSource=abc123",
			sourceLabel: "KUBECOVE_KUBECONFIG + 2 paths",
			showSourceLabels: true,
			warnings: [],
		};
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				return sources as T;
			},
		};

		expect(await getKubeconfigSources(client)).toEqual(sources);
		expect(await setKubeconfigEnvVar(client, "KUBECOVE_KUBECONFIG")).toEqual(
			sources,
		);
		expect(await setShowKubeconfigSourceLabels(client, false)).toEqual(sources);
		expect(await addKubeconfigPaths(client, ["/tmp/a", "/tmp/b"])).toEqual(
			sources,
		);
		expect(await removeKubeconfigPath(client, "/tmp/a")).toEqual(sources);
		expect(await reorderKubeconfigPaths(client, ["/tmp/b", "/tmp/a"])).toEqual(
			sources,
		);

		expect(calls).toEqual([
			{ cmd: "get_kubeconfig_sources", args: undefined },
			{
				cmd: "set_kubeconfig_env_var",
				args: { envVar: "KUBECOVE_KUBECONFIG" },
			},
			{ cmd: "set_show_kubeconfig_source_labels", args: { show: false } },
			{ cmd: "add_kubeconfig_paths", args: { paths: ["/tmp/a", "/tmp/b"] } },
			{ cmd: "remove_kubeconfig_path", args: { path: "/tmp/a" } },
			{
				cmd: "reorder_kubeconfig_paths",
				args: { paths: ["/tmp/b", "/tmp/a"] },
			},
		]);
	});

	test("passes namespace requests through the typed client", async () => {
		const mockNamespaces: NamespaceSummary[] = [
			{ name: "default", age: "1d" },
		];
		const client = createMockTauriClient({ list_namespaces: mockNamespaces });

		expect(await listNamespaces(client, "minikube")).toEqual(mockNamespaces);
	});

	test("returns raw YAML strings from the resource YAML wrapper", async () => {
		const mockYaml = "apiVersion: v1\nkind: Pod\nmetadata:\n  name: test-pod";
		const client = createMockTauriClient({ get_resource_yaml: mockYaml });

		expect(
			await getResourceYaml(client, "minikube", "Pod", "test-pod", "default"),
		).toBe(mockYaml);
	});

	test("passes topology scope through the typed client", async () => {
		const topology: ResourceTopology = { nodes: [], edges: [], warnings: [] };
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				return topology as T;
			},
		};

		expect(await listResourceTopology(client, "kind-dev", ["default", "payments"], "networkFlow")).toEqual(topology);
		expect(calls).toEqual([
			{
				cmd: "list_resource_topology",
				args: {
					clusterContext: "kind-dev",
					namespaces: ["default", "payments"],
					mode: "networkFlow",
				},
			},
		]);
	});

	test("coalesces duplicate in-flight topology requests", async () => {
		const topology: ResourceTopology = { nodes: [], edges: [], warnings: [] };
		let calls = 0;
		const client = {
			invoke: async <T>(): Promise<T> => {
				calls += 1;
				await new Promise((resolve) => setTimeout(resolve, 1));
				return topology as T;
			},
		};

		const [first, second] = await Promise.all([
			listResourceTopology(client, "kind-dev", ["payments", "default"], "ownership"),
			listResourceTopology(client, "kind-dev", ["default", "payments"], "ownership"),
		]);

		expect(first).toBe(topology);
		expect(second).toBe(topology);
		expect(calls).toBe(1);
	});

	test("does not coalesce cancellable topology requests", async () => {
		let calls = 0;
		const seen: Record<string, unknown>[] = [];
		const client = {
			invoke: async <T>(_cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls += 1;
				if (args) seen.push(args);
				await new Promise((resolve) => setTimeout(resolve, 1));
				return { nodes: [], edges: [], warnings: [] } as T;
			},
		};

		await Promise.all([
			listResourceTopology(client, "kind-dev", ["payments"], "ownership", undefined, {
				requestId: "first",
				cancelScope: "topology",
			}),
			listResourceTopology(client, "kind-dev", ["payments"], "ownership", undefined, {
				requestId: "second",
				cancelScope: "topology",
			}),
		]);

		expect(calls).toBe(2);
		expect(seen.map((args) => args.requestId)).toEqual(["first", "second"]);
	});

	test("coalesces duplicate in-flight present custom resource requests", async () => {
		const kinds = [
			{
				kind: "Cluster",
				apiVersion: "postgresql.cnpg.io/v1",
				group: "postgresql.cnpg.io",
				version: "v1",
				plural: "clusters",
				namespaced: true,
			},
		];
		let calls = 0;
		const client = {
			invoke: async <T>(): Promise<T> => {
				calls += 1;
				await new Promise((resolve) => setTimeout(resolve, 1));
				return kinds as T;
			},
		};

		const [first, second] = await Promise.all([
			listPresentCustomResourceKinds(client, "kind-dev", ["cnpg-system", "default"]),
			listPresentCustomResourceKinds(client, "kind-dev", ["default", "cnpg-system"]),
		]);

		expect(first).toBe(kinds);
		expect(second).toBe(kinds);
		expect(calls).toBe(1);
	});

	test("fetches app usage metrics through the typed client", async () => {
		const metrics: AppUsageMetrics = {
			cpuPercent: 2.4,
			memoryBytes: 184 * 1024 * 1024,
			processCount: 3,
			sampledAt: "2026-05-20T10:00:00Z",
			breakdown: [
				{
					label: "WebView",
					description: "Embedded WebView browser runtime",
					cpuPercent: 1.8,
					memoryBytes: 128 * 1024 * 1024,
					processCount: 2,
					children: [
						{
							label: "WebView process 1",
							description: "Embedded WebView browser runtime",
							cpuPercent: 1.2,
							memoryBytes: 96 * 1024 * 1024,
							processCount: 1,
							children: [],
						},
					],
				},
			],
		};
		const client = createMockTauriClient({ get_app_usage_metrics: metrics });

		expect(await getAppUsageMetrics(client)).toEqual(metrics);
	});

	test("passes backend diagnostics commands through typed wrappers", async () => {
		const event: BackendDiagnosticEvent = {
			id: 1,
			recordedAt: "2026-06-15T10:00:00Z",
			command: "list_resource_scope",
			status: "ok",
			durationMs: 42,
			summary: [{ key: "rows", value: "12" }],
		};
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				if (cmd === "get_backend_diagnostics") return [event] as T;
				if (cmd === "set_backend_diagnostics_enabled") return true as T;
				return undefined as T;
			},
		};

		expect(await setBackendDiagnosticsEnabled(client, true)).toBe(true);
		expect(await getBackendDiagnostics(client)).toEqual([event]);
		await clearBackendDiagnostics(client);

		expect(calls).toEqual([
			{
				cmd: "set_backend_diagnostics_enabled",
				args: { enabled: true },
			},
			{ cmd: "get_backend_diagnostics", args: undefined },
			{ cmd: "clear_backend_diagnostics", args: undefined },
		]);
	});

	test("passes resource metrics scope through the typed client", async () => {
		const metrics: ResourceMetricsSummary = {
			cluster: "kind-dev",
			availability: { status: "available", message: "metrics available" },
			pods: [
				{
					kind: "Pod",
					cluster: "kind-dev",
					name: "api-0",
					namespace: "payments",
					cpuMillicores: 125,
					memoryBytes: 128,
					sourcePods: [],
				},
			],
			nodes: [],
			workloads: [],
			warnings: [],
		};
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				return metrics as T;
			},
		};

		expect(await listResourceMetrics(client, "kind-dev", ["payments"])).toEqual(metrics);
		expect(calls).toEqual([
			{
				cmd: "list_resource_metrics",
				args: { clusterContext: "kind-dev", namespaces: ["payments"] },
			},
		]);
	});

	test("passes Helm release requests through typed wrappers", async () => {
		const release: HelmReleaseSummary = {
			cluster: "kind-dev",
			name: "payments",
			namespace: "payments",
			age: "5m",
			chart: "payments-1.2.3",
			appVersion: "2026.5.21",
			revision: 7,
			status: "deployed",
			storageKind: "Secret",
			storageName: "sh.helm.release.v1.payments.v7",
		};
		const details: HelmReleaseDetails = {
			summary: release,
			yaml: "kind: Secret",
			metadata: { name: release.storageName },
			valuesSummary: {
				hasValues: true,
				topLevelKeys: ["image"],
				valueCount: 1,
			},
			manifestSummary: {
				resourceCount: 1,
				resources: [
					{
						apiVersion: "apps/v1",
						kind: "Deployment",
						name: "payments",
						namespace: "payments",
					},
				],
				truncated: false,
			},
		};
		const reconciliation: HelmReleaseReconciliation = {
			summary: release,
			totals: {
				tracked: 1,
				unlabeledLive: 0,
				missing: 0,
				labelOnly: 0,
				unavailable: 0,
			},
			resources: [
				{
					apiVersion: "apps/v1",
					kind: "Deployment",
					name: "payments",
					namespace: "payments",
					status: "tracked",
					statusMessage:
						"Manifest resource exists and carries the explicit Helm release label.",
					inManifest: true,
					explicitHelmLabel: true,
				},
			],
			warnings: [],
		};
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				if (cmd === "list_helm_releases") return [release] as T;
				if (cmd === "get_helm_release_reconciliation") {
					return reconciliation as T;
				}
				return details as T;
			},
		};

		expect(await listHelmReleases(client, "kind-dev")).toEqual([release]);
		expect(await getHelmReleaseDetails(client, release)).toEqual(details);
		expect(await getHelmReleaseReconciliation(client, release)).toEqual(
			reconciliation,
		);
		expect(calls).toEqual([
			{
				cmd: "list_helm_releases",
				args: { clusterContext: "kind-dev" },
			},
			{
				cmd: "get_helm_release_details",
				args: {
					clusterContext: "kind-dev",
					namespace: "payments",
					storageKind: "Secret",
					storageName: "sh.helm.release.v1.payments.v7",
				},
			},
			{
				cmd: "get_helm_release_reconciliation",
				args: {
					clusterContext: "kind-dev",
					namespace: "payments",
					storageKind: "Secret",
					storageName: "sh.helm.release.v1.payments.v7",
				},
			},
		]);
	});

	test("passes Flux requests through typed wrappers", async () => {
		const resourceKind: FluxResourceKind = {
			group: "kustomize.toolkit.fluxcd.io",
			version: "v1",
			apiVersion: "kustomize.toolkit.fluxcd.io/v1",
			kind: "Kustomization",
			plural: "kustomizations",
			namespaced: true,
			category: "Kustomize",
		};
		const detection: FluxDetectionSummary = {
			detected: true,
			kinds: [resourceKind],
			missingKinds: [],
		};
		const summary: FluxResourceSummary = {
			cluster: "kind-dev",
			resourceKind,
			name: "apps",
			namespace: "flux-system",
			age: "5m",
			readyStatus: "True",
			inventory: [],
		};
		const details: FluxResourceDetails = {
			summary,
			yaml: "kind: Kustomization",
			metadata: { name: "apps" },
			status: { conditions: [] },
		};
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				if (cmd === "detect_flux") return detection as T;
				if (cmd === "list_flux_resources") return [summary] as T;
				return details as T;
			},
		};

		expect(await detectFlux(client, "kind-dev", "KUBECONFIG")).toEqual(detection);
		expect(
			await listFluxResources(client, "kind-dev", resourceKind, "KUBECONFIG"),
		).toEqual([summary]);
		expect(
			await getFluxResourceDetails(
				client,
				"kind-dev",
				resourceKind,
				"apps",
				"flux-system",
				"KUBECONFIG",
				"clean",
				"yaml",
			),
		).toEqual(details);
		expect(calls).toEqual([
			{
				cmd: "detect_flux",
				args: { clusterContext: "kind-dev", kubeconfigEnvVar: "KUBECONFIG" },
			},
			{
				cmd: "list_flux_resources",
				args: {
					clusterContext: "kind-dev",
					resourceKind,
					kubeconfigEnvVar: "KUBECONFIG",
				},
			},
			{
				cmd: "get_flux_resource_details",
				args: {
					clusterContext: "kind-dev",
					resourceKind,
					name: "apps",
					namespace: "flux-system",
					kubeconfigEnvVar: "KUBECONFIG",
					yamlViewMode: "clean",
					yamlEncoding: "yaml",
				},
			},
		]);
	});

	test("passes RBAC inspection scope through typed wrappers", async () => {
		const inspection: RbacInspectionSummary = {
			cluster: "kind-dev",
			warnings: [],
			serviceAccounts: [],
			roles: [],
			clusterRoles: [],
			roleBindings: [],
			clusterRoleBindings: [],
			namespaceAccess: [],
		};
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				return inspection as T;
			},
		};

		expect(await listRbacInspection(client, "kind-dev", ["payments"])).toEqual(inspection);
		expect(calls).toEqual([
			{
				cmd: "list_rbac_inspection",
				args: { clusterContext: "kind-dev", namespaces: ["payments"] },
			},
		]);
	});

	test("passes port-forward requests through typed wrappers", async () => {
		const session: PortForwardSessionSummary = {
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
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				if (cmd === "start_pod_port_forward") return session as T;
				if (cmd === "list_port_forwards") return [session] as T;
				return true as T;
			},
		};
		const request = {
			clusterContext: "kind-dev",
			namespace: "payments",
			targetKind: "Pod" as const,
			targetName: "api-0",
			podName: "api-0",
			remotePort: 8080,
			localPort: 18080,
		};
		const serviceRequest = {
			clusterContext: "kind-dev",
			namespace: "payments",
			targetKind: "Service" as const,
			targetName: "api",
			remotePort: 8080,
			localPort: 18080,
		};

		expect(await startPortForward(client, request)).toEqual(session);
		expect(await startPortForward(client, serviceRequest)).toEqual(session);
		expect(await startPodPortForward(client, request)).toEqual(session);
		expect(await listPortForwards(client)).toEqual([session]);
		expect(await stopPodPortForward(client, session.id)).toBe(true);
		expect(calls).toEqual([
			{
				cmd: "start_pod_port_forward",
				args: { request },
			},
			{
				cmd: "start_pod_port_forward",
				args: { request: serviceRequest },
			},
			{
				cmd: "start_pod_port_forward",
				args: { request },
			},
			{
				cmd: "list_port_forwards",
				args: undefined,
			},
			{
				cmd: "stop_port_forward",
				args: { sessionId: session.id },
			},
		]);
	});

	test("strips backend source keys from stream and live-session requests", async () => {
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				if (cmd === "start_pod_log_stream") return "stream-1" as T;
				return {
					id: "session-1",
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
				} as T;
			},
		};
		const channel = {} as never;

		await startPodLogStream(
			client,
			{
				clusterContext: "kind-dev",
				kubeconfigEnvVar: "kubeconfigSource=abc123",
				namespace: "payments",
				podName: "api-0",
				tailLines: 100,
			},
			channel,
		);
		await startPortForward(client, {
			clusterContext: "kind-dev",
			kubeconfigEnvVar: "kubeconfigSource=abc123",
			namespace: "payments",
			targetKind: "Pod",
			targetName: "api-0",
			podName: "api-0",
			remotePort: 8080,
		});
		await startPodExecSession(
			client,
			{
				clusterContext: "kind-dev",
				kubeconfigEnvVar: "kubeconfigSource=abc123",
				namespace: "payments",
				podName: "api-0",
				command: ["/bin/sh"],
				stdin: true,
				tty: true,
				terminalSize: { cols: 100, rows: 32 },
				confirmation: {
					acknowledged: true,
					target: "kind-dev/payments/Pod/api-0",
					command: "/bin/sh",
				},
			},
			channel,
		);

		expect(calls[0]?.args).toEqual({
			request: {
				clusterContext: "kind-dev",
				namespace: "payments",
				podName: "api-0",
				tailLines: 100,
			},
			channel,
		});
		expect(calls[1]?.args).toEqual({
			request: {
				clusterContext: "kind-dev",
				namespace: "payments",
				targetKind: "Pod",
				targetName: "api-0",
				podName: "api-0",
				remotePort: 8080,
			},
		});
		expect(calls[2]?.args).toEqual({
			request: {
				clusterContext: "kind-dev",
				namespace: "payments",
				podName: "api-0",
				command: ["/bin/sh"],
				stdin: true,
				tty: true,
				terminalSize: { cols: 100, rows: 32 },
				confirmation: {
					acknowledged: true,
					target: "kind-dev/payments/Pod/api-0",
					command: "/bin/sh",
				},
			},
			channel,
		});
	});
});

describe("isAppError", () => {
	test("accepts serialized app errors and rejects partial objects", () => {
		expect(isAppError({ message: "test", kind: "cluster" })).toBe(true);
		expect(isAppError({ message: "test" })).toBe(false);
		expect(isAppError({ kind: "cluster" })).toBe(false);
		expect(isAppError(null)).toBe(false);
	});
});
