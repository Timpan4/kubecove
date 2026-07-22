import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { browser, expect } from "@wdio/globals";
import { after, describe, it } from "mocha";

type Resource = { name: string; namespace?: string };
type Session = { id: string; localUrl?: string };
type StreamMessage = { type: string; line?: string; action?: string; target?: { name?: string } };

const cluster = `${process.env.KUBECOVE_E2E_CLUSTER}-admin`;
const kubectl = process.env.KUBECOVE_E2E_KUBECTL;
const kubeconfig = process.env.KUBECOVE_KUBECONFIG;
const streams: string[] = [];
const sessions: string[] = [];
const execFileAsync = promisify(execFile);

async function invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
	return await browser.execute(
		async (payload) => {
			const tauri = (window as unknown as { __TAURI__: { core: { invoke: (name: string, value: unknown) => Promise<unknown> } } }).__TAURI__;
			return await tauri.core.invoke(payload.command, payload.args);
		},
		{ command, args },
	) as T;
}

async function runKubectl(args: string[]) {
	if (!kubectl || !kubeconfig) throw new Error("runner did not provide kubectl isolation");
	const { stdout } = await execFileAsync(kubectl, args, { env: { ...process.env, KUBECONFIG: kubeconfig } });
	return stdout.trim();
}

async function startChannel(command: string, args: Record<string, unknown>, bucket: string) {
	return await browser.execute(
		async (payload) => {
			const tauri = (window as unknown as { __TAURI__: { core: { Channel: new () => { onmessage: (message: unknown) => void }; invoke: (name: string, value: unknown) => Promise<string> } } }).__TAURI__;
			const target = window as unknown as Record<string, unknown>;
			target[payload.bucket] = [];
			const channel = new tauri.core.Channel();
			channel.onmessage = (message) => (target[payload.bucket] as unknown[]).push(message);
			return await tauri.core.invoke(payload.command, { ...payload.args, channel });
		},
		{ command, args, bucket },
	) as string;
}

async function messages(bucket: string): Promise<StreamMessage[]> {
	return await browser.execute((name) => ((window as unknown as Record<string, unknown>)[name] ?? []) as StreamMessage[], bucket) as StreamMessage[];
}

describe("native Kind command boundary", () => {
	after(async () => {
		for (const id of streams) await invoke("stop_stream", { streamId: id }).catch(() => undefined);
		for (const id of sessions) await invoke("stop_port_forward", { sessionId: id }).catch(() => undefined);
	});

	it("discovers the empty-profile contexts, namespaces, and dynamic kinds", async () => {
		await expect(browser.$("body")).toHaveText(expect.stringContaining(cluster));
		await browser.$("#workspace-name").setValue("Kind E2E Lab");
		const create = await browser.$("button=Create workspace");
		await create.waitForEnabled({ timeout: 20_000 });
		await create.click();
		await expect(browser.$("body")).toHaveText(expect.stringContaining("Kind E2E Lab"));
		const contexts = await invoke<Array<{ name: string }>>("list_kube_contexts");
		expect(contexts.map(({ name }) => name).sort()).toEqual([`${process.env.KUBECOVE_E2E_CLUSTER}-admin`, `${process.env.KUBECOVE_E2E_CLUSTER}-restricted`].sort());
		const namespaces = await invoke<Array<{ name: string }>>("list_namespaces", { clusterContext: cluster });
		expect(namespaces.map(({ name }) => name)).toContain("e2e-discovery");
		const kinds = await invoke<Array<{ kind: string }>>("list_resource_kinds", { clusterContext: cluster });
		expect(kinds.map(({ kind }) => kind)).toContain("Application");
	});

	it("receives external create and delete through one live watch", async () => {
		const bucket = "__kubecoveWatchMessages";
		const id = await startChannel("start_resource_watch", { clusterContext: cluster, keys: [{ resourceKind: { kind: "ConfigMap" }, namespace: "e2e-watch" }] }, bucket);
		streams.push(id);
		await runKubectl(["create", "configmap", "external-watch-marker", "-n", "e2e-watch", "--from-literal=marker=created"]);
		await browser.waitUntil(async () => (await messages(bucket)).some(({ type, target }) => type === "resourceChanged" && target?.name === "external-watch-marker"), { timeout: 20_000, interval: 200 });
		await runKubectl(["delete", "configmap", "external-watch-marker", "-n", "e2e-watch", "--wait=true"]);
		await browser.waitUntil(async () => (await messages(bucket)).filter(({ type, target }) => type === "resourceChanged" && target?.name === "external-watch-marker").length >= 2, { timeout: 20_000, interval: 200 });
	});

	it("previews and applies selected-resource YAML, then verifies independently", async () => {
		const yaml = await invoke<string>("get_resource_yaml", { clusterContext: cluster, kind: "ConfigMap", name: "apply-target", namespace: "e2e-apply", yamlViewMode: "applyClean" });
		const changed = yaml.replace("marker: before-apply", "marker: before-apply\n  e2eApplied: after-apply");
		const request = { clusterContext: cluster, kind: "ConfigMap", apiVersion: "v1", namespaced: true, name: "apply-target", namespace: "e2e-apply", yaml: changed };
		const preview = await invoke<{ currentYaml: string; dryRunYaml: string }>("prepare_yaml_apply", { request });
		expect(preview.currentYaml).toContain("before-apply");
		expect(preview.dryRunYaml).toContain("after-apply");
		await invoke("apply_yaml", { request });
		expect(await runKubectl(["get", "configmap", "apply-target", "-n", "e2e-apply", "-o", "jsonpath={.data.e2eApplied}"])).toBe("after-apply");
	});

	it("surfaces restricted apply as forbidden and preserves the target", async () => {
		const restricted = `${process.env.KUBECOVE_E2E_CLUSTER}-restricted`;
		const request = { clusterContext: restricted, kind: "ConfigMap", apiVersion: "v1", namespaced: true, name: "restricted-target", namespace: "e2e-restricted", yaml: "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: restricted-target\n  namespace: e2e-restricted\ndata:\n  marker: changed\n" };
		let error = "";
		try { await invoke("prepare_yaml_apply", { request }); } catch (value) { error = String(value); }
		expect(error.toLowerCase()).toContain("forbidden");
		expect(await runKubectl(["get", "configmap", "restricted-target", "-n", "e2e-restricted", "-o", "jsonpath={.data.marker}"])).toBe("unchanged");
	});

	it("streams logs, executes the exact Pod, and serves then stops a Service forward", async () => {
		const pods = await invoke<Resource[]>("list_resources", { clusterContext: cluster, kind: "Pod", namespace: "e2e-sessions" });
		const pod = pods.find(({ name }) => name.startsWith("fixture-api-"));
		expect(pod?.name).toBeTruthy();
		const logBucket = "__kubecoveLogMessages";
		streams.push(await startChannel("start_pod_log_stream", { request: { clusterContext: cluster, namespace: "e2e-sessions", podName: pod?.name, container: "api", tailLines: 20 } }, logBucket));
		await browser.waitUntil(async () => (await messages(logBucket)).some(({ line }) => line?.includes("kubecove-log-marker")), { timeout: 20_000, interval: 200 });
		const execBucket = "__kubecoveExecMessages";
		const command = ["sh", "-c", "echo kubecove-exec-marker"];
		await startChannel("start_pod_exec_session", { request: { clusterContext: cluster, namespace: "e2e-sessions", podName: pod?.name, container: "api", command, stdin: false, tty: false, terminalSize: { cols: 80, rows: 24 }, confirmation: { acknowledged: true, target: `${cluster}/e2e-sessions/Pod/${pod?.name}/container/api`, command: JSON.stringify(command) } } }, execBucket);
		await browser.waitUntil(async () => JSON.stringify(await messages(execBucket)).includes("kubecove-exec-marker"), { timeout: 20_000, interval: 200 });
		const forward = await invoke<Session>("start_pod_port_forward", { request: { clusterContext: cluster, namespace: "e2e-sessions", targetKind: "Service", targetName: "fixture-api", remotePort: 8080 } });
		sessions.push(forward.id);
		await browser.waitUntil(async () => (await fetch(forward.localUrl as string)).ok, { timeout: 20_000, interval: 200 });
		expect(await (await fetch(forward.localUrl as string)).text()).toContain("kubecove-http-marker");
		expect(await invoke<boolean>("stop_port_forward", { sessionId: forward.id })).toBe(true);
		sessions.splice(sessions.indexOf(forward.id), 1);
	});

	it("lists Argo objects and the installed Helm release", async () => {
		const apps = await invoke<Resource[]>("list_argocd_applications", { clusterContext: cluster });
		expect(apps.map(({ name }) => name)).toContain("fixture-app");
		const releases = await invoke<Array<{ name: string }>>("list_helm_releases", { clusterContext: cluster });
		expect(releases.map(({ name }) => name)).toContain("fixture-chart");
	});
});
