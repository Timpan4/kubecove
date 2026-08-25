import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { downloadAsset, verifyAsset } from "./assets";
import { kindConfig, kindDeleteArgs } from "./cluster";
import { safeDiagnosticCommands, safeDiagnosticText } from "./diagnostics";
import { gitSeedIdentity, kindMetricsManifest } from "./git-seed";
import { bootstrapOrder, platformApplicationNames, readinessPhase, tenantApplicationNames } from "./lab";
import { assertOwned, assertOwnedOnDisk, expectedCluster, type Ownership, ownershipFromDisk } from "./ownership";
import { chartPins, validateImmutablePins } from "./platform";

const record: Ownership = { kind: "run", runId: "run-1", cluster: "kubecove-e2e-run-1", dir: "/tmp/run-1", raw: "/tmp/run-1/kind.raw.kubeconfig", kubeconfig: "/tmp/run-1/kubeconfig", dataDir: "/tmp/run-1/data", kindConfig: "/tmp/run-1/kind.yaml", disableDefaultCNI: true, provider: "docker", kubernetes: "1.35" };

describe("Kind harness", () => {
	test("uses Cilium instead of Kind default CNI while retaining kube-proxy", () => {
		expect(kindConfig()).toContain("disableDefaultCNI: true");
		expect(kindConfig()).not.toContain("disableKubeProxy");
	});
	test("requires immutable platform pins", () => {
		expect(() => validateImmutablePins()).not.toThrow();
		expect(chartPins.argocd).toMatchObject({ version: "10.1.4", appVersion: "3.4.5" });
		expect(chartPins.traefik).toMatchObject({ version: "41.0.2", appVersion: "3.7.6" });
	});
	test("retries transient asset downloads twice", async () => {
		const responses: Array<Response | Error> = [
			new Error("connection reset"),
			new Response(null, { status: 503 }),
			new Response("fixture"),
		];
		const bytes = await downloadAsset("fixture", "https://example.invalid", async () => {
			const response = responses.shift();
			if (response instanceof Error) throw response;
			if (!response) throw new Error("unexpected retry");
			return response;
		});
		expect(new TextDecoder().decode(bytes)).toBe("fixture");
		expect(responses).toHaveLength(0);
	});
	test("does not retry deterministic asset download failures", async () => {
		let calls = 0;
		await expect(
			downloadAsset("fixture", "https://example.invalid", async () => {
				calls += 1;
				return new Response(null, { status: 404 });
			}),
		).rejects.toThrow("download failed: fixture (404)");
		expect(calls).toBe(1);
	});
	test("verifies asset bytes and Kind metrics arguments", () => {
		expect(verifyAsset("fixture", new TextEncoder().encode("fixture"), "f16d05ec6b29248d2c61adb1e9263f78e4f7bace1b955014a2d17872cfe4064d")).toHaveLength(64);
		expect(() => verifyAsset("fixture", new Uint8Array(), "0".repeat(64))).toThrow("checksum mismatch");
		expect(kindMetricsManifest("        - --kubelet-use-node-status-port\n")).toContain("--kubelet-insecure-tls");
	});
	test("uses a fixed seed identity and strict bootstrap order", () => {
		expect(gitSeedIdentity.email).toBe("e2e@kubecove.invalid");
		expect(bootstrapOrder).toEqual(["kind", "cilium", "git", "argocd", "root-application", "platform-applications", "tenant-applications", "operations-helm-release"]);
	});
	test("refuses ownership records outside exact run", () => {
		expect(() => assertOwned(record, "run", record.dir, record.runId, "workspace")).not.toThrow();
		expect(() => assertOwned({ ...record, cluster: expectedCluster("run", "other", "workspace") }, "run", record.dir, record.runId, "workspace")).toThrow("refuse operation");
		expect(() => assertOwned({ ...record, dataDir: "/tmp/elsewhere" }, "run", record.dir, record.runId, "workspace")).toThrow("refuse operation");
	});
	test("upgrades ownership records written before kindConfig existed", () => {
		const { kindConfig: _, ...legacy } = record;
		expect(ownershipFromDisk(legacy, "run", record.dir, record.runId, "workspace").kindConfig).toBe(join(record.dir, "kind.yaml"));
	});
	test.skipIf(process.platform === "win32")("refuses symlinked ownership paths", async () => {
		const dir = await mkdtemp(join(tmpdir(), "kubecove-owned-"));
		const outside = await mkdtemp(join(tmpdir(), "kubecove-outside-"));
		const owned = { ...record, dir, raw: join(dir, "kind.raw.kubeconfig"), kubeconfig: join(dir, "kubeconfig"), dataDir: join(dir, "data"), kindConfig: join(dir, "kind.yaml") };
		try {
			await writeFile(owned.raw, "raw"); await writeFile(owned.kubeconfig, "config"); await writeFile(owned.kindConfig, "kind"); await mkdir(owned.dataDir);
			await rm(owned.kubeconfig); await writeFile(join(outside, "kubeconfig"), "config"); await symlink(join(outside, "kubeconfig"), owned.kubeconfig);
			await expect(assertOwnedOnDisk(owned, "run", dir, owned.runId, "workspace")).rejects.toThrow("refuse symlinked ownership path");
		} finally { await rm(dir, { recursive: true, force: true }); await rm(outside, { recursive: true, force: true }); }
	});
	test("refuses legacy dev records without CNI proof but permits explicit cleanup", () => {
		const legacy = { ...record, kind: "dev" as const, runId: "workspace", cluster: expectedCluster("dev", "workspace", "workspace"), disableDefaultCNI: undefined };
		expect(() => ownershipFromDisk(legacy, "dev", legacy.dir, legacy.runId, "workspace")).toThrow("run bun run dev:kind:down");
		expect(() => ownershipFromDisk(legacy, "dev", legacy.dir, legacy.runId, "workspace", false)).not.toThrow();
	});
	test("preserves immutable selectors used by persistent dev labs", () => {
		const fixtures = readFileSync(new URL("../fixtures/all.yaml", import.meta.url), "utf8");
		expect(fixtures).toContain("selector: {matchLabels: {app: discovery-api}}");
		expect(fixtures).toContain("selector: {matchLabels: {app: fixture-api}}");
	});
	test("deletes only the exact cluster through the isolated kubeconfig", () => {
		expect(kindDeleteArgs(record.cluster, record.raw)).toEqual(["delete", "cluster", "--name", record.cluster, "--kubeconfig", record.raw]);
		expect(() => kindDeleteArgs("", record.raw)).toThrow("exact cluster cleanup");
	});
	test("wait order classifies exact platform and tenant applications", () => {
		expect(platformApplicationNames).toEqual(["platform-argocd", "platform-cilium", "platform-metrics", "platform-storage", "platform-ingress"]);
		expect(tenantApplicationNames).toEqual(["tenant-catalog", "tenant-ledger"]);
		expect(readinessPhase("application", "Healthy")).toBe("ready");
		expect(readinessPhase("deployment", "Progressing")).toBe("waiting");
	});
	test("diagnostics omit secrets and redact credential-shaped text", () => {
		expect(safeDiagnosticCommands.flat().join(" ")).not.toContain("secrets");
		expect(safeDiagnosticCommands.flat().join(" ")).not.toContain("-o yaml");
		expect(safeDiagnosticText("token: value\nclient-key-data: value")).toBe("token: REDACTED\nclient-key-data: REDACTED");
	});
});
