import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { browser, expect } from "@wdio/globals";
import { after, describe, it } from "mocha";
import type { ArgoApplicationSummary } from "../../../src/lib/gitops-types";
import type { HelmReleaseSummary } from "../../../src/lib/helm-types";
import type {
	ClusterContext,
	DiscoveredResourceKind,
	NamespaceSummary,
	PodExecSessionMessage,
	PodExecSessionRequest,
	PodExecSessionSummary,
	PodLogStreamRequest,
	PortForwardRequest,
	PortForwardSessionSummary,
	ResourceSummary,
	StreamMessage,
	WatchResourceKey,
	YamlApplyPreview,
	YamlApplyRequest,
	YamlApplyResult,
} from "../../../src/lib/types";

type CommandMap = {
	list_kube_contexts: { args: Record<string, never>; result: ClusterContext[] };
	list_namespaces: { args: { clusterContext: string }; result: NamespaceSummary[] };
	list_resource_kinds: { args: { clusterContext: string }; result: DiscoveredResourceKind[] };
	get_resource_yaml: { args: { clusterContext: string; kind: string; name: string; namespace: string; yamlViewMode: "applyClean" }; result: string };
	prepare_yaml_apply: { args: { request: YamlApplyRequest }; result: YamlApplyPreview };
	apply_yaml: { args: { request: YamlApplyRequest }; result: YamlApplyResult };
	list_resources: { args: { clusterContext: string; kind: string; namespace: string }; result: ResourceSummary[] };
	start_pod_port_forward: { args: { request: PortForwardRequest }; result: PortForwardSessionSummary };
	stop_port_forward: { args: { sessionId: string }; result: boolean };
	stop_pod_exec_session: { args: { sessionId: string }; result: boolean };
	stop_stream: { args: { streamId: string }; result: boolean };
	list_argocd_applications: { args: { clusterContext: string }; result: ArgoApplicationSummary[] };
	list_helm_releases: { args: { clusterContext: string }; result: HelmReleaseSummary[] };
};
type ChannelCommandMap = {
	start_resource_watch: { args: { clusterContext: string; keys: WatchResourceKey[] }; result: string };
	start_pod_log_stream: { args: { request: PodLogStreamRequest }; result: string };
	start_pod_exec_session: { args: { request: PodExecSessionRequest }; result: PodExecSessionSummary };
};

const clusterName = process.env.KUBECOVE_E2E_CLUSTER;
if (!clusterName) throw new Error("runner did not provide KUBECOVE_E2E_CLUSTER");
const cluster = `${clusterName}-admin`;
const kubectl = process.env.KUBECOVE_E2E_KUBECTL;
const kubeconfig = process.env.KUBECOVE_KUBECONFIG;
const streams: string[] = [];
const sessions: string[] = [];
const execSessions: string[] = [];
const execFileAsync = promisify(execFile);

async function invokeTauri<K extends keyof CommandMap>(command: K, args: CommandMap[K]["args"]): Promise<CommandMap[K]["result"]> {
	return await browser.execute(
		async (payload) => {
			const tauri = (window as unknown as { __TAURI__: { core: { invoke: (name: string, value: unknown) => Promise<unknown> } } }).__TAURI__;
			return await tauri.core.invoke(payload.command, payload.args);
		},
		{ command, args },
	) as CommandMap[K]["result"];
}

async function runKubectl(args: string[]) {
	if (!kubectl || !kubeconfig) throw new Error("runner did not provide kubectl isolation");
	const { stdout } = await execFileAsync(kubectl, args, { env: { ...process.env, KUBECONFIG: kubeconfig } });
	return stdout.trim();
}

async function invokeChannel<K extends keyof ChannelCommandMap>(command: K, args: ChannelCommandMap[K]["args"], bucket: string): Promise<ChannelCommandMap[K]["result"]> {
	return await browser.execute(
		async (payload) => {
			const tauri = (window as unknown as { __TAURI__: { core: { Channel: new () => { onmessage: (message: unknown) => void }; invoke: (name: string, value: unknown) => Promise<unknown> } } }).__TAURI__;
			const target = window as unknown as Record<string, unknown>;
			target[payload.bucket] = [];
			const channel = new tauri.core.Channel();
			channel.onmessage = (message) => (target[payload.bucket] as unknown[]).push(message);
			return await tauri.core.invoke(payload.command, { ...payload.args, channel });
		},
		{ command, args, bucket },
	) as ChannelCommandMap[K]["result"];
}

async function messages<T extends StreamMessage | PodExecSessionMessage>(bucket: string): Promise<T[]> {
	return await browser.execute((name) => ((window as unknown as Record<string, unknown>)[name] ?? []) as unknown[], bucket) as T[];
}

const tauri = {
	listKubeContexts: () => invokeTauri("list_kube_contexts", {}),
	listNamespaces: (clusterContext: string) => invokeTauri("list_namespaces", { clusterContext }),
	listResourceKinds: (clusterContext: string) => invokeTauri("list_resource_kinds", { clusterContext }),
	getResourceYaml: (args: CommandMap["get_resource_yaml"]["args"]) => invokeTauri("get_resource_yaml", args),
	prepareYamlApply: (request: YamlApplyRequest) => invokeTauri("prepare_yaml_apply", { request }),
	applyYaml: (request: YamlApplyRequest) => invokeTauri("apply_yaml", { request }),
	listResources: (args: CommandMap["list_resources"]["args"]) => invokeTauri("list_resources", args),
	startPodPortForward: (request: PortForwardRequest) => invokeTauri("start_pod_port_forward", { request }),
	stopPortForward: (sessionId: string) => invokeTauri("stop_port_forward", { sessionId }),
	stopPodExecSession: (sessionId: string) => invokeTauri("stop_pod_exec_session", { sessionId }),
	stopStream: (streamId: string) => invokeTauri("stop_stream", { streamId }),
	listArgoCdApplications: (clusterContext: string) => invokeTauri("list_argocd_applications", { clusterContext }),
	listHelmReleases: (clusterContext: string) => invokeTauri("list_helm_releases", { clusterContext }),
	startResourceWatch: (clusterContext: string, keys: WatchResourceKey[], bucket: string) => invokeChannel("start_resource_watch", { clusterContext, keys }, bucket),
	startPodLogStream: (request: PodLogStreamRequest, bucket: string) => invokeChannel("start_pod_log_stream", { request }, bucket),
	startPodExecSession: (request: PodExecSessionRequest, bucket: string) => invokeChannel("start_pod_exec_session", { request }, bucket),
};

describe("native Kind command boundary", () => {
	after(async () => {
		for (const id of streams) await tauri.stopStream(id).catch(() => undefined);
		for (const id of execSessions) await tauri.stopPodExecSession(id).catch(() => undefined);
		for (const id of sessions) await tauri.stopPortForward(id).catch(() => undefined);
	});

	it("discovers the empty-profile contexts, namespaces, and dynamic kinds", async () => {
		await expect(browser.$("body")).toHaveText(expect.stringContaining(cluster));
		await browser.$("#workspace-name").setValue("Kind E2E Lab");
		const create = await browser.$("button=Create workspace");
		await create.waitForEnabled({ timeout: 20_000 });
		await create.click();
		await expect(browser.$("body")).toHaveText(expect.stringContaining("Kind E2E Lab"));
		const contexts = await tauri.listKubeContexts();
		expect(contexts.map(({ name }) => name).sort()).toEqual([`${clusterName}-admin`, `${clusterName}-restricted`].sort());
		const namespaces = await tauri.listNamespaces(cluster);
		expect(namespaces.map(({ name }) => name)).toContain("e2e-discovery");
		const kinds = await tauri.listResourceKinds(cluster);
		expect(kinds.map(({ kind }) => kind)).toContain("Application");
	});

	it("receives external create and delete through one live watch", async () => {
		const bucket = "__kubecoveWatchMessages";
		const id = await tauri.startResourceWatch(cluster, [{ resourceKind: { kind: "ConfigMap" }, namespace: "e2e-watch" }], bucket);
		streams.push(id);
		await runKubectl(["create", "configmap", "external-watch-marker", "-n", "e2e-watch", "--from-literal=marker=created"]);
		await browser.waitUntil(async () => (await messages<StreamMessage>(bucket)).some((message) => message.type === "resourceChanged" && message.target.name === "external-watch-marker"), { timeout: 20_000, interval: 200 });
		await runKubectl(["delete", "configmap", "external-watch-marker", "-n", "e2e-watch", "--wait=true"]);
		await browser.waitUntil(async () => (await messages<StreamMessage>(bucket)).filter((message) => message.type === "resourceChanged" && message.target.name === "external-watch-marker").length >= 2, { timeout: 20_000, interval: 200 });
	});

	it("previews and applies selected-resource YAML, then verifies independently", async () => {
		const yaml = await tauri.getResourceYaml({ clusterContext: cluster, kind: "ConfigMap", name: "apply-target", namespace: "e2e-apply", yamlViewMode: "applyClean" });
		const changed = yaml.replace("marker: before-apply", "marker: before-apply\n  e2eApplied: after-apply");
		const request = { clusterContext: cluster, kind: "ConfigMap", apiVersion: "v1", namespaced: true, name: "apply-target", namespace: "e2e-apply", yaml: changed };
		const preview = await tauri.prepareYamlApply(request);
		expect(preview.currentYaml).toContain("before-apply");
		expect(preview.dryRunYaml).toContain("after-apply");
		await tauri.applyYaml(request);
		expect(await runKubectl(["get", "configmap", "apply-target", "-n", "e2e-apply", "-o", "jsonpath={.data.e2eApplied}"])).toBe("after-apply");
	});

	it("surfaces restricted apply as forbidden and preserves the target", async () => {
		const restricted = `${clusterName}-restricted`;
		const request = { clusterContext: restricted, kind: "ConfigMap", apiVersion: "v1", namespaced: true, name: "restricted-target", namespace: "e2e-restricted", yaml: "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: restricted-target\n  namespace: e2e-restricted\ndata:\n  marker: changed\n" };
		let error = "";
		try { await tauri.prepareYamlApply(request); } catch (value) { error = String(value); }
		expect(error.toLowerCase()).toContain("forbidden");
		expect(await runKubectl(["get", "configmap", "restricted-target", "-n", "e2e-restricted", "-o", "jsonpath={.data.marker}"])).toBe("unchanged");
	});

	it("streams logs, executes the exact Pod, and serves then stops a Service forward", async () => {
		const pods = await tauri.listResources({ clusterContext: cluster, kind: "Pod", namespace: "e2e-sessions" });
		const pod = pods.find(({ name }) => name.startsWith("fixture-api-"));
		if (!pod) throw new Error("fixture-api Pod was not discovered");
		const logBucket = "__kubecoveLogMessages";
		streams.push(await tauri.startPodLogStream({ clusterContext: cluster, namespace: "e2e-sessions", podName: pod.name, container: "api", tailLines: 20 }, logBucket));
		await browser.waitUntil(async () => (await messages<StreamMessage>(logBucket)).some((message) => message.type === "logLine" && message.line.includes("kubecove-log-marker")), { timeout: 20_000, interval: 200 });
		const execBucket = "__kubecoveExecMessages";
		const command = ["sh", "-c", "echo kubecove-exec-marker"];
		const exec = await tauri.startPodExecSession({ clusterContext: cluster, namespace: "e2e-sessions", podName: pod.name, container: "api", command, stdin: false, tty: false, terminalSize: { cols: 80, rows: 24 }, confirmation: { acknowledged: true, target: `${cluster}/e2e-sessions/Pod/${pod.name}/container/api`, command: JSON.stringify(command) } }, execBucket);
		execSessions.push(exec.id);
		await browser.waitUntil(async () => JSON.stringify(await messages<PodExecSessionMessage>(execBucket)).includes("kubecove-exec-marker"), { timeout: 20_000, interval: 200 });
		const forward = await tauri.startPodPortForward({ clusterContext: cluster, namespace: "e2e-sessions", targetKind: "Service", targetName: "fixture-api", remotePort: 8080 });
		sessions.push(forward.id);
		await browser.waitUntil(async () => (await fetch(forward.localUrl as string)).ok, { timeout: 20_000, interval: 200 });
		expect(await (await fetch(forward.localUrl as string)).text()).toContain("kubecove-http-marker");
		expect(await tauri.stopPortForward(forward.id)).toBe(true);
		sessions.splice(sessions.indexOf(forward.id), 1);
	});

	it("lists Argo objects and the installed Helm release", async () => {
		const apps = await tauri.listArgoCdApplications(cluster);
		expect(apps.map(({ name }) => name)).toContain("fixture-app");
		const releases = await tauri.listHelmReleases(cluster);
		expect(releases.map(({ name }) => name)).toContain("fixture-chart");
	});
});
