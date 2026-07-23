import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { safeDiagnosticCommands, safeDiagnosticText } from "./diagnostics";
import { verifyAsset } from "./assets";
import { kindConfig, kindDeleteArgs } from "./cluster";
import { gitSeedIdentity, kindMetricsManifest } from "./git-seed";
import { bootstrapOrder, platformApplicationNames, readinessPhase, tenantApplicationNames } from "./lab";
import { assertOwned, expectedCluster, ownershipFromDisk, type Ownership } from "./ownership";
import { chartPins, validateImmutablePins } from "./platform";

const record: Ownership = { kind: "run", runId: "run-1", cluster: "kubecove-e2e-run-1", dir: "/tmp/run-1", raw: "/tmp/run-1/kind.raw.kubeconfig", kubeconfig: "/tmp/run-1/kubeconfig", dataDir: "/tmp/run-1/data", kindConfig: "/tmp/run-1/kind.yaml", provider: "docker", kubernetes: "1.35" };

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
		expect(ownershipFromDisk(legacy, "run", record.dir, record.runId, "workspace").kindConfig).toBe(record.kindConfig);
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
