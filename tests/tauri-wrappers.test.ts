import { describe, expect, test } from "bun:test";
import { queryKeys } from "../src/lib/queryKeys";
import { kubeconfigSourceKey } from "../src/lib/settings";
import {
	getDynamicResourceDetails,
	getResourceYaml,
	isAppError,
	isTauriRuntime,
	listKubeContexts,
	listNamespaces,
	listPresentCustomResourceKinds,
	listResourceTopology,
	revealSecretDataValue,
	shouldUseBrowserDevMocks,
	startPodExecSession,
	startPodLogStream,
	startPortForward,
} from "../src/lib/tauri";
import {
	discoverArgoServers,
	getArgoApplicationInspector,
	getArgoResourceComparison,
} from "../src/lib/tauri-argo";
import type { JsonObject, JsonValue, ResourceTopology } from "../src/lib/types";

function mockInvokeResult<T>(value: JsonValue): T {
	// SAFETY: each test provides JSON-shaped mock data for the exact wrapper result asserted below.
	return value as T;
}

describe("runtime detection and Argo cancellation", () => {
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

	test("passes cancellation identity through Argo inspection reads", async () => {
		const calls: Array<{ cmd: string; args?: JsonObject }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: JsonObject) => {
				calls.push({ cmd, args });
				return mockInvokeResult<T>({});
			},
		};
		const request = {
			clusterContext: "kind-dev",
			transport: "connected" as const,
			connectionId: "profile-1",
			application: { name: "demo", workspaceId: "workspace-1" },
		};
		const cancellation = { cancelScope: "argo:demo", requestId: "request-1" };

		await discoverArgoServers(client, "kind-dev", "KUBECONFIG", cancellation);
		await getArgoApplicationInspector(client, request, cancellation);
		await getArgoResourceComparison(
			client,
			{ ...request, resource: { kind: "Deployment", name: "api" } },
			cancellation,
		);

		expect(calls.map((call) => call.args)).toEqual([
			{
				clusterContext: "kind-dev",
				kubeconfigEnvVar: "KUBECONFIG",
				...cancellation,
			},
			{ ...request, ...cancellation },
			{
				...request,
				resource: { kind: "Deployment", name: "api" },
				...cancellation,
			},
		]);
	});
});

describe("typed Tauri wrappers", () => {
	test("omits backend source keys from kubeconfig env var args", async () => {
		const calls: Array<{ cmd: string; args?: JsonObject }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: JsonObject): Promise<T> => {
				calls.push({ cmd, args });
				return mockInvokeResult<T>([]);
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
	});

	test("keeps broad Secret YAML redacted and reveals only selected keys", async () => {
		const calls: Array<{ cmd: string; args?: JsonObject }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: JsonObject): Promise<T> => {
				calls.push({ cmd, args });
				return mockInvokeResult<T>(cmd === "reveal_secret_data_value" ? "c2VjcmV0" : "data:\n  token: <redacted>");
			},
		};

		await getResourceYaml(client, "minikube", "Secret", "api", "default");
		expect(await revealSecretDataValue(client, "minikube", "api", "default", "token")).toBe("c2VjcmV0");
		expect(calls).toEqual([
			{ cmd: "get_resource_yaml", args: { clusterContext: "minikube", kind: "Secret", name: "api", namespace: "default", yamlViewMode: undefined, yamlEncoding: undefined } },
			{ cmd: "reveal_secret_data_value", args: { clusterContext: "minikube", name: "api", namespace: "default", key: "token" } },
		]);
	});

	test("does not send caller-controlled Secret redaction for dynamic details", async () => {
		const calls: Array<{ cmd: string; args?: JsonObject }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: JsonObject): Promise<T> => {
				calls.push({ cmd, args });
				return mockInvokeResult<T>({});
			},
		};

		await getDynamicResourceDetails(client, "minikube", {
			group: "",
			version: "v1",
			apiVersion: "v1",
			kind: "Secret",
			plural: "secrets",
			namespaced: true,
		}, "api", "default");
		expect(calls[0]?.args).not.toHaveProperty("redactSecrets");
	});

	test("coalesces duplicate in-flight topology requests", async () => {
		const topology: ResourceTopology = { nodes: [], edges: [], warnings: [] };
		let calls = 0;
		const client = {
			invoke: async <T>(): Promise<T> => {
				calls += 1;
				await new Promise((resolve) => setTimeout(resolve, 1));
				return mockInvokeResult<T>(topology);
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
		const seen: JsonObject[] = [];
		const client = {
			invoke: async <T>(_cmd: string, args?: JsonObject): Promise<T> => {
				calls += 1;
				if (args) seen.push(args);
				await new Promise((resolve) => setTimeout(resolve, 1));
				return mockInvokeResult<T>({ nodes: [], edges: [], warnings: [] });
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
				return mockInvokeResult<T>(kinds);
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

	test("strips backend source keys from stream and live-session requests", async () => {
		const calls: Array<{ cmd: string; args?: JsonObject }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: JsonObject): Promise<T> => {
				calls.push({ cmd, args });
				if (cmd === "start_pod_log_stream") return mockInvokeResult<T>("stream-1");
				return mockInvokeResult<T>({
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
				});
			},
		};
		// SAFETY: this wrapper test verifies serialized request arguments only; mocked invoke never reads channel.
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
