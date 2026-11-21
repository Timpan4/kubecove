import { createHash, randomUUID } from "node:crypto";
import { once } from "node:events";
import { existsSync } from "node:fs";
import { chmod, mkdir, mkdtemp, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import { createServer as createHttpServer } from "node:http";
import { arch, platform, tmpdir } from "node:os";
import { basename, delimiter, dirname, join, resolve } from "node:path";
import { parse, stringify } from "yaml";
import type { ViteDevServer } from "vite";
import { buildBrowserServer, createWdioRunner, runDevBrowser, runServerWdio, writeSmokeConfig } from "./harness/browser-run";
import { startMemoryFixture } from "./harness/memory-fixture";
import { downloadAsset, sha256, verifyAsset } from "./harness/assets";
import { kindConfig, kindDeleteArgs } from "./harness/cluster";
import { safeDiagnosticCommands, safeDiagnosticText } from "./harness/diagnostics";
import { gitSeedIdentity, prepareGitSeed } from "./harness/git-seed";
import { gitRepositoryUrl, platformApplicationNames, tenantApplicationNames } from "./harness/lab";
import { captureProcessOutput, cleanEnvironment, type InterruptSignal, processOutput, trackChild } from "./harness/lifecycle-processes";
import { assertOwned, assertOwnedOnDisk, assertOwnedPathOnDisk, expectedCluster, type Ownership, ownershipFromDisk, type Provider } from "./harness/ownership";
import { chartPins, fixturePaths, gitDaemonPins, validateImmutablePins } from "./harness/platform";

type Kubeconfig = {
	apiVersion?: string;
	clusters?: Array<{ name?: string; cluster: { server?: string } }>;
	contexts?: Array<{ name: string; context: { cluster: string } }>;
	users?: Array<{ user: { token?: string } }>;
};

const root = resolve(process.cwd());
const stateDir = join(root, ".e2e");
const args = Bun.argv.slice(3);
const option = (name: string) => {
	const index = args.indexOf(name);
	return index >= 0 ? args[index + 1] : undefined;
};
const action = Bun.argv[2];
const profiling = action === "desktop-profile";
const memoryFixture = option("--memory-fixture");
if (memoryFixture && (!profiling || (memoryFixture !== "normal" && memoryFixture !== "large"))) throw new Error("--memory-fixture requires desktop-profile and normal or large");
const keep = args.includes("--keep");
const requestedRunId = option("--run-id") ?? process.env.E2E_RUN_ID;
const generatedRunId = `${Date.now()}-${process.pid}-${randomUUID().slice(0, 8)}`;
const runId = requestedRunId ?? generatedRunId;
const requestedKubernetes = option("--kubernetes") ?? "1.35";
const requestedProvider = option("--provider") ?? "auto";
const workspaceHash = createHash("sha256").update(root).digest("hex").slice(0, 12);
const os = platform() === "darwin" ? "darwin" : platform() === "win32" ? "windows" : "linux";
const cpu = arch() === "arm64" ? "arm64" : "amd64";
const suffix = os === "windows" ? ".exe" : "";

if (!/^[a-zA-Z0-9][a-zA-Z0-9-]{0,47}$/.test(runId)) throw new Error("run ID must be 1-48 letters, digits, or hyphens");
const kubernetes = parseKubernetesVersion(requestedKubernetes);
validateImmutablePins();

const images = {
	"1.34": "kindest/node:v1.34.11@sha256:44e222ee2132dab25ff87301682f89eb82c7880ea3a1bf543bfe9708fd08d67d",
	"1.35": "kindest/node:v1.35.8@sha256:07b2536e30b803ed61d1677a79df6115f798ce64c80f9e22f6ed45afd09323c0",
	"1.36": "kindest/node:v1.36.4@sha256:099e049362a1526b2db71494e1947aae99bd16290d7c895f2b7ea312e3cbfaed",
	"1.37": "kindest/node:v1.37.0@sha256:a1ed56cfb0e7b93589bdf97c8cd566405a265939e3620fc4f5de89adff580ae5",
} satisfies Record<string, string>;
function parseKubernetesVersion(value: string): keyof typeof images {
	if (value === "1.34" || value === "1.35" || value === "1.36" || value === "1.37") return value;
	throw new Error("--kubernetes must be 1.34, 1.35, 1.36, or 1.37");
}
function imageFor(version: string) {
	return images[parseKubernetesVersion(version)];
}
// kubectl supports one minor of version skew: v1.35 covers clusters 1.34–1.36 and v1.36 covers 1.37.
const kubectlRelease = kubernetes === "1.37" ? "v1.36.5" : "v1.35.9";
const kubectlPins = {
	"v1.35.9": { "darwin-amd64": "51a130a5ea965314a41d27e840e96a7f4e4c13c508ab5fa0fd101ee4c1829990", "darwin-arm64": "652548e6a9405266c5acd2719703bc8d36befa46e79e151a0d646904aa32c89b", "linux-amd64": "3cfeaf80be482b435b0aa214aff6e0b2c312ee23c0ff20810c75517b6004c6eb", "linux-arm64": "39c98bca82875d9a9ddfb6f3c5ab17c3f78ea0801230683e538a67d5c053308d", "windows-amd64": "160bebf50d49ac4b5892370803da09cc26bbeabed6a4cf22bf382bd72d14d4d7" },
	"v1.36.5": { "darwin-amd64": "03338828c3ba74c37bf5964573fbfbab132eb5bb15da2f821c25a4043d6eb3f0", "darwin-arm64": "c5850a4a6b9469b26cce47c1ab456285f58e308283dcdf72b4ddda3db49b0ec0", "linux-amd64": "33bc88a24c3b09cf55bfd59e6ca977881b61fc3f25ab65bc80c65b430248648e", "linux-arm64": "88fc1aca8fd0c1b44fe379f67c39bd6858bdfc50241dc55775f84d0f5b31da3a", "windows-amd64": "5e87156aa0b20e3c1c66db22a11d898f9a5f787513a3bbd8df2633c207a5a855" },
} as const;
const pins = {
	kind: { "darwin-amd64": "5a99f26f57246dc9319dd294803313197a0f34d33c525b3ea8b655db5916ece0", "darwin-arm64": "0c8c7dbe5e23594a198b786c4bc13dacc101fa6196b0cb0b23a1ca44e61f4b4f", "linux-amd64": "aee6151561422756b764a4ae28e7f44cda5af5a9eead3cc9985112b1de8d8e0d", "linux-arm64": "20022bee6cfcd5086cb7234d218e3454e6090022f2a8f55d1fa7fcf42c3867a2", "windows-amd64": "4b22adaa135368c5a465d56bbd8e520cbea87272a06ca00b6078e7b81515c9fc" },
	kubectl: kubectlPins[kubectlRelease],
	helm: { "darwin-amd64": "34bec4eba761ba5c16d1965ee6b8f5b0abe5993974a802274e2a4f0418893d89", "darwin-arm64": "6b4e61c8cd27d99e5a801e63ff25dd5c70b36875c2cd659d3e9ce6b76fc23198", "linux-amd64": "0403a88ae20913d11b10a2467b50087721fea7581736da6b9d3992f5357fe54b", "linux-arm64": "8cf8f66ecbc252ac8f7f06cca68aba88508ddfde3a6a34feb1678ad1694b4f88", "windows-amd64": "fcfd7b796dcf0f225abc40b5636a3b01f55f3de2fb1dfb18475df94e62c14112" },
} as const;
const obscuraRelease = "v0.2.5";
const obscuraArchives = {
	"darwin-amd64": { name: "obscura-x86_64-macos.tar.gz", sha256: "9f354cf929a577491d6ba0ef451e07baefffd19d34115330f69a0e463771d6cd" },
	"darwin-arm64": { name: "obscura-aarch64-macos.tar.gz", sha256: "fcca31eba4280cd67a2e226ef4b015e432d5c089f78cefd074ad12bbc8e23345" },
	"linux-amd64": { name: "obscura-x86_64-linux.tar.gz", sha256: "2116764b99bc084fae3668b68aa535b4cf456e4498845b47493ad4123e83ca36" },
	"linux-arm64": { name: "obscura-aarch64-linux.tar.gz", sha256: "24c549a348b1d464e80aa3f643bc0c874b0a2942fb7836c683528028d0fbb190" },
	"windows-amd64": { name: "obscura-x86_64-windows.zip", sha256: "9e56fa1cc1f9e4fb78926f7e9f36c40166b65177541ad7b7fd9645944c49174e" },
} as const;

const runDir = (id = runId) => join(stateDir, "runs", id);
const devDir = join(stateDir, "dev", workspaceHash);
const recordPath = (dir: string) => join(dir, "ownership.json");
const pinKeys = {
	darwin: { amd64: "darwin-amd64", arm64: "darwin-arm64" },
	linux: { amd64: "linux-amd64", arm64: "linux-arm64" },
	windows: { amd64: "windows-amd64", arm64: "windows-amd64" },
} as const;
const key = () => pinKeys[os][cpu];

const children = new Set<(signal: InterruptSignal) => Promise<void>>();
const cancellation = new AbortController();
let interruptedBy: InterruptSignal | undefined;
let stoppingChildren: Promise<void> | undefined;
async function execute(cmd: string, commandArgs: string[], extra: Record<string, string | undefined> = {}, capture = false, logFile?: string) {
	cancellation.signal.throwIfAborted();
	return runCommand(cmd, commandArgs, extra, capture, logFile, true);
}
async function runCommand(cmd: string, commandArgs: string[], extra: Record<string, string | undefined>, capture: boolean, logFile?: string, track = false) {
	const child = Bun.spawn([cmd, ...commandArgs], { cwd: root, detached: os !== "windows", env: cleanEnvironment(extra), stdout: capture || logFile ? "pipe" : "inherit", stderr: capture || logFile ? "pipe" : "inherit" });
	const stdout = captureProcessOutput(child.stdout);
	const stderr = captureProcessOutput(child.stderr);
	const complete = (signal?: AbortSignal) => Promise.all([child.exited, stdout.drain(signal), stderr.drain(signal)]);
	const tracked = track ? trackChild(child, complete) : undefined;
	if (tracked) children.add(tracked.stop);
	try {
		const [code] = await (tracked?.done ?? complete()).finally(async () => {
			if (logFile) { await mkdir(dirname(logFile), { recursive: true }); await writeFile(logFile, `${stdout.text()}${stderr.text()}`); }
		});
		if (track) cancellation.signal.throwIfAborted();
		if (code !== 0) throw new Error(`${basename(cmd)} failed (${code})`);
		return { stdout: stdout.text(), stderr: stderr.text() };
	} finally { if (tracked) children.delete(tracked.stop); }
}
async function attempt(cmd: string, commandArgs: string[], extra: Record<string, string | undefined> = {}) {
	try { return await execute(cmd, commandArgs, extra, true); } catch { cancellation.signal.throwIfAborted(); return undefined; }
}
const runWdio = createWdioRunner(root, option("--spec"), execute);
async function verified(name: string, url: string, expected: string) {
	cancellation.signal.throwIfAborted();
	const target = join(stateDir, "tools", name);
	await mkdir(dirname(target), { recursive: true });
	if (existsSync(target) && sha256(new Uint8Array(await readFile(target))) === expected) return target;
	await rm(target, { force: true });
	const bytes = await downloadAsset(name, url, (source) => fetch(source, { signal: cancellation.signal }));
	cancellation.signal.throwIfAborted();
	verifyAsset(name, bytes, expected);
	const temporary = `${target}.${process.pid}.part`;
	await writeFile(temporary, bytes); await rename(temporary, target);
	if (os !== "windows") await chmod(target, 0o755);
	return target;
}
async function obscuraBinary() {
	const asset = obscuraArchives[key()];
	const archive = await verified(`${obscuraRelease}-${asset.name}`, `https://github.com/h4ckf0r0day/obscura/releases/download/${obscuraRelease}/${asset.name}`, asset.sha256);
	const destination = join(stateDir, "tools", `obscura-${obscuraRelease}-${key()}`);
	await mkdir(destination, { recursive: true });
	if (os === "windows") {
		const escapedArchive = archive.replaceAll("'", "''");
		const escapedDestination = destination.replaceAll("'", "''");
		await execute("powershell", ["-NoProfile", "-Command", `Expand-Archive -Force -LiteralPath '${escapedArchive}' -DestinationPath '${escapedDestination}'`]);
	} else await execute("tar", ["-xzf", archive, "-C", destination]);
	const binary = join(destination, `obscura${suffix}`);
	if (!existsSync(binary)) throw new Error("Obscura archive did not contain the executable");
	if (os !== "windows") await chmod(binary, 0o755);
	return binary;
}
async function runObscuraFast(env: Record<string, string>, profile: string) {
	const binary = await obscuraBinary();
	cancellation.signal.throwIfAborted();
	// Port 0 asks the OS to reserve an unused loopback port for this browser.
	const child = Bun.spawn([binary, "serve", "--host", "127.0.0.1", "--port", "0", "--allow-private-network", "--storage-dir", profile], {
		cwd: root, detached: os !== "windows", env: cleanEnvironment({ ...env, RUST_LOG: "obscura_cdp::server=info", OBSCURA_CDP_TOKEN: undefined }), stdout: "ignore", stderr: "pipe",
	});
	const log = Bun.file(join(env.KUBECOVE_E2E_ARTIFACTS, "obscura-server.log")).writer();
	const { promise: readiness, resolve: ready, reject: failed } = Promise.withResolvers<string>();
	// Match the existing fast suite's Mocha timeout, including a server that never binds.
	const startupTimer = setTimeout(() => failed(new Error("Obscura did not start within the fast suite timeout")), 30_000);
	const tracked = trackChild(child, async (signal) => {
		const decoder = new TextDecoder();
		let tail = "";
		try {
			for await (const chunk of processOutput(child.stderr, signal)) {
				log.write(chunk);
				tail += decoder.decode(chunk, { stream: true });
				const lastLine = tail.lastIndexOf("\n");
				if (lastLine === -1) continue;
				const endpoint = /Obscura CDP server listening on (ws:\/\/127\.0\.0\.1:[1-9]\d*)\b/.exec(tail.slice(0, lastLine))?.[1];
				if (endpoint) ready(`${endpoint}/devtools/browser`);
				tail = tail.slice(lastLine + 1);
			}
			failed(new Error("Obscura exited before its CDP server was ready; see obscura-server.log"));
		} catch (error) { failed(new Error("Could not read Obscura startup logs", { cause: error })); }
		finally { await log.end(); }
		await child.exited;
	});
	children.add(tracked.stop);
	void tracked.done.catch(failed);
	let failure: { error: unknown } | undefined;
	try {
		const endpoint = await readiness;
		clearTimeout(startupTimer);
		await execute("bun", ["scripts/obscura-smoke.ts"], { ...env, OBSCURA_CDP_URL: endpoint });
	} catch (error) {
		failure = { error };
	} finally { clearTimeout(startupTimer); }
	try {
		await tracked.stop("SIGTERM");
	} catch (error) {
		if (!failure) throw error;
		console.error("Could not stop Obscura", error);
	} finally { children.delete(tracked.stop); }
	if (failure) throw failure.error;
}
let toolPromise: Promise<{ kind: string; kubectl: string; helm: string }> | undefined;
function tools() {
	if (toolPromise) return toolPromise;
	toolPromise = (async () => {
	if (!pins.kind[key()] || !pins.kubectl[key()] || !pins.helm[key()]) throw new Error(`unsupported tool platform ${key()}`);
	const kind = await verified(`kind${suffix}`, `https://github.com/kubernetes-sigs/kind/releases/download/v0.33.0/kind-${os}-${cpu}`, pins.kind[key()]);
	const kubectl = await verified(`kubectl-${kubectlRelease}${suffix}`, `https://dl.k8s.io/release/${kubectlRelease}/bin/${os}/${cpu}/kubectl${suffix}`, pins.kubectl[key()]);
	const archiveName = `helm-v4.3.1-${os}-${cpu}.${os === "windows" ? "zip" : "tar.gz"}`;
	const archive = await verified(archiveName, `https://get.helm.sh/${archiveName}`, pins.helm[key()]);
	const extractDir = join(stateDir, "tools", `helm-extract-${process.pid}`); await rm(extractDir, { recursive: true, force: true }); await mkdir(extractDir, { recursive: true });
	if (os === "windows") { const escapedArchive = archive.replaceAll("'", "''"); const escapedDir = extractDir.replaceAll("'", "''"); await execute("powershell", ["-NoProfile", "-Command", `Expand-Archive -Force -LiteralPath '${escapedArchive}' -DestinationPath '${escapedDir}'`]); }
	else await execute("tar", ["-xzf", archive, "-C", extractDir]);
	const helm = join(stateDir, "tools", `helm${suffix}`); await Bun.write(helm, Bun.file(join(extractDir, `${os}-${cpu}`, `helm${suffix}`))); if (os !== "windows") await chmod(helm, 0o755); await rm(extractDir, { recursive: true, force: true });
		return { kind, kubectl, helm };
	})();
	return toolPromise;
}

function providerBinary(name: Provider) { return Bun.which(name); }
async function providerEnvironment(name: Provider) { const binary = providerBinary(name); if (!binary) throw new Error(`${name} is unavailable`); return { binary, env: { KIND_EXPERIMENTAL_PROVIDER: name, PATH: `${dirname(binary)}${delimiter}${process.env.PATH ?? ""}` } }; }
async function ensureProvider(name: Provider) { const selected = await providerEnvironment(name); if (await attempt(selected.binary, ["info"])) return selected; if (name === "podman") await execute(selected.binary, ["machine", "start"]); if (!await attempt(selected.binary, ["info"])) throw new Error(`${name} is installed but unavailable`); return selected; }
async function selectProvider() {
	if (requestedProvider === "docker" || requestedProvider === "podman") return ensureProvider(requestedProvider);
	if (requestedProvider !== "auto") throw new Error("--provider must be auto, docker, or podman");
	for (const candidate of ["docker", "podman"] as const) { if (!providerBinary(candidate)) continue; try { return await ensureProvider(candidate); } catch {} }
	throw new Error("no usable Docker or Podman provider found");
}

function parseKubeconfig(source: string): Kubeconfig {
	// SAFETY: callers validate the parsed kubeconfig shape before using ownership or endpoint fields.
	return parse(source) as Kubeconfig;
}
function parseOwnership(source: string): Partial<Ownership> {
	// SAFETY: `ownershipFromDisk` validates every required ownership field before any filesystem action.
	return parse(source) as Partial<Ownership>;
}
async function readOwnership(file: string, kind: Ownership["kind"], dir: string, id: string, requireDefaultCni = kind === "dev") { await assertOwnedPathOnDisk(file, dir); const record = ownershipFromDisk(parseOwnership(await readFile(file, "utf8")), kind, dir, id, workspaceHash, requireDefaultCni); await assertOwnedOnDisk(record, kind, dir, id, workspaceHash); return record; }
async function clusterExists(kind: string, cluster: string, provider: Awaited<ReturnType<typeof ensureProvider>>) { const result = await attempt(kind, ["get", "clusters"], provider.env); return result?.stdout.split(/\r?\n/).includes(cluster) ?? false; }
async function removeCluster(record: Ownership) {
	const dir = record.kind === "run" ? runDir(record.runId) : devDir; await assertOwnedOnDisk(record, record.kind, dir, record.runId, workspaceHash);
	const { kind } = await tools();
	const provider = action === "cleanup" || action === "dev-down" ? await ensureProvider(record.provider) : await providerEnvironment(record.provider);
	// Teardown runs after the work has settled, even when execute has been cancelled.
	const { stdout } = await runCommand(kind, ["get", "clusters"], provider.env, true);
	if (stdout.split(/\r?\n/).includes(record.cluster)) await runCommand(kind, kindDeleteArgs(record.cluster, record.raw), provider.env, false);
	await rm(dir, { recursive: true, force: true });
}

function finalKubeconfig(config: Kubeconfig, record: Ownership, token: string) {
	if (config.clusters?.length !== 1 || config.users?.length !== 1) throw new Error("unexpected Kind kubeconfig shape");
	return { apiVersion: config.apiVersion ?? "v1", kind: "Config", clusters: [{ name: record.cluster, cluster: config.clusters[0].cluster }], contexts: ["admin", "restricted"].map((role) => ({ name: `${record.cluster}-${role}`, context: { cluster: record.cluster, user: `${record.cluster}-${role}` } })), "current-context": `${record.cluster}-admin`, users: [{ name: `${record.cluster}-admin`, user: config.users[0].user }, { name: `${record.cluster}-restricted`, user: { token } }] };
}
function validateGeneratedConfig(config: Kubeconfig, record: Ownership) {
	const contexts = config.contexts?.map(({ name }) => name).sort() ?? [];
	const expected = [`${record.cluster}-admin`, `${record.cluster}-restricted`].sort();
	if (JSON.stringify(contexts) !== JSON.stringify(expected) || config.clusters?.length !== 1 || config.clusters[0].name !== record.cluster || config.contexts?.some(({ context }) => context.cluster !== record.cluster)) throw new Error("generated kubeconfig scope mismatch");
	const server = new URL(config.clusters[0].cluster.server ?? ""); if (!["127.0.0.1", "localhost", "::1"].includes(server.hostname)) throw new Error("generated API server is not loopback");
}
async function bootstrap(record: Ownership, kubectl: string) {
	await assertOwnedOnDisk(record, record.kind, record.dir, record.runId, workspaceHash);
	const rawEnv = { KUBECONFIG: record.raw };
	await execute(kubectl, ["apply", "-f", fixturePaths.rbac], rawEnv);
	const token = (await execute(kubectl, ["create", "token", "restricted", "-n", "e2e-system", "--duration=2h"], rawEnv, true)).stdout.trim();
	const config = finalKubeconfig(parseKubeconfig(await readFile(record.raw, "utf8")), record, token); validateGeneratedConfig(config, record);
	await writeFile(record.kubeconfig, stringify(config)); if (os !== "windows") await chmod(record.kubeconfig, 0o600);
}

async function applyFile(kubectl: string, file: string, env: Record<string, string | undefined>, serverSide = false) {
	await execute(kubectl, ["apply", ...(serverSide ? ["--server-side", "--force-conflicts"] : []), "-f", file], env);
}
async function verifyPlatformSources() {
	for (const [name, pin] of Object.entries(chartPins)) await verified(`platform-${name}-${pin.version}`, "repository" in pin ? pin.repository : pin.manifest, pin.sha256);
}
async function applyTemplate(record: Ownership, helm: string, kubectl: string, name: string, pin: typeof chartPins.cilium | typeof chartPins.argocd | typeof chartPins.traefik, namespace: string, values: string) {
	const chart = await verified(`${name}-${pin.version}.tgz`, pin.repository, pin.sha256);
	const rendered = await execute(helm, ["template", name, chart, "--namespace", namespace, "--values", values, "--include-crds"], { KUBECONFIG: record.kubeconfig }, true);
	const output = join(record.dir, `${name}.yaml`); await writeFile(output, rendered.stdout);
	await applyFile(kubectl, output, { KUBECONFIG: record.kubeconfig }, true);
}
async function materialize(record: Ownership, name: string, source: string, seedCommit = "") {
	const target = join(record.dir, name); const text = await readFile(source, "utf8");
	await writeFile(target, text.replaceAll("__KUBECOVE_GIT_REPO_URL__", gitRepositoryUrl).replaceAll("__KUBECOVE_SEED_COMMIT__", seedCommit));
	return target;
}
async function namespaceFile(record: Ownership, name: string) {
	const target = join(record.dir, `namespace-${name}.yaml`);
	await writeFile(target, `apiVersion: v1\nkind: Namespace\nmetadata:\n  name: ${name}\n`);
	return target;
}
async function seedRepository(record: Ownership, kubectl: string, env: Record<string, string | undefined>) {
	const metrics = await verified(`platform-metricsServer-${chartPins.metricsServer.version}`, chartPins.metricsServer.manifest, chartPins.metricsServer.sha256);
	const storage = await verified(`platform-localPath-${chartPins.localPath.version}`, chartPins.localPath.manifest, chartPins.localPath.sha256);
	const gitDaemonPin = gitDaemonPins[cpu];
	const gitDaemonPackage = await verified(`git-daemon-${gitDaemonPin.version}-${cpu}.apk`, gitDaemonPin.url, gitDaemonPin.sha256);
	const worktree = join(record.dir, "git-seed");
	await prepareGitSeed({ source: fixturePaths.gitops, destination: worktree, repositoryUrl: gitRepositoryUrl, metricsManifest: await readFile(metrics, "utf8"), storageManifest: await readFile(storage, "utf8") });
	for (const [name, pin] of [["argocd", chartPins.argocd], ["cilium", chartPins.cilium], ["traefik", chartPins.traefik]] as const) {
		const archive = await verified(`platform-${name}-${pin.version}`, pin.repository, pin.sha256);
		const destination = join(worktree, "vendor", name);
		await mkdir(destination, { recursive: true });
		await execute("tar", ["-xzf", archive, "-C", destination, "--strip-components=1"]);
	}
	for (const path of ["bootstrap", "platform/metrics", "platform/storage", "tenants/catalog", "tenants/ledger"]) await execute(kubectl, ["kustomize", join(worktree, path)], env, true);
	await execute("git", ["-C", worktree, "init", "--initial-branch=main"], {}, true);
	await execute("git", ["-C", worktree, "config", "user.name", gitSeedIdentity.name]);
	await execute("git", ["-C", worktree, "config", "user.email", gitSeedIdentity.email]);
	await execute("git", ["-C", worktree, "add", "."]);
	const gitDates = { GIT_AUTHOR_DATE: "2000-01-01T00:00:00Z", GIT_COMMITTER_DATE: "2000-01-01T00:00:00Z" };
	await execute("git", ["-C", worktree, "commit", "--no-gpg-sign", "-m", "Seed KubeCove production-shaped lab"], gitDates, true);
	const commit = (await execute("git", ["-C", worktree, "rev-parse", "HEAD"], {}, true)).stdout.trim();
	const bundle = join(record.dir, "repository.bundle");
	await execute("git", ["-C", worktree, "bundle", "create", bundle, "main"], {}, true);
	if ((await stat(bundle)).size > 1_000_000) throw new Error("verified Git seed exceeds the Kubernetes ConfigMap limit");
	const configMap = await execute(kubectl, ["create", "configmap", "gitops-repository", "-n", "e2e-system", `--from-file=repository.bundle=${bundle}`, "--dry-run=client", "-o", "yaml"], env, true);
	const configMapFile = join(record.dir, "gitops-repository.yaml");
	await writeFile(configMapFile, configMap.stdout);
	await writeFile(join(record.dir, "seed-commit.txt"), `${commit}\n`);
	await applyFile(kubectl, configMapFile, env, true);
	const packageConfigMap = await execute(kubectl, ["create", "configmap", "git-daemon-package", "-n", "e2e-system", `--from-file=git-daemon.apk=${gitDaemonPackage}`, "--dry-run=client", "-o", "yaml"], env, true);
	const packageConfigMapFile = join(record.dir, "git-daemon-package.yaml");
	await writeFile(packageConfigMapFile, packageConfigMap.stdout);
	await applyFile(kubectl, packageConfigMapFile, env, true);
	return commit;
}
async function waitApplication(kubectl: string, name: string, env: Record<string, string | undefined>) {
	await execute(kubectl, ["wait", "--for=jsonpath={.status.sync.status}=Synced", `application/${name}`, "-n", "argocd", "--timeout=300s"], env);
	await execute(kubectl, ["wait", "--for=jsonpath={.status.health.status}=Healthy", `application/${name}`, "-n", "argocd", "--timeout=300s"], env);
}
async function poll(description: string, timeoutMs: number, check: () => Promise<boolean>) {
	const deadline = Date.now() + timeoutMs;
	do {
		cancellation.signal.throwIfAborted();
		if (await check()) return;
		await Bun.sleep(1_000);
	} while (Date.now() < deadline);
	throw new Error(`timed out waiting for ${description}`);
}
async function applyLab(record: Ownership) {
	await assertOwnedOnDisk(record, record.kind, record.dir, record.runId, workspaceHash);
	const { kubectl, helm } = await tools(); const env = { KUBECONFIG: record.kubeconfig };
	await verifyPlatformSources();
	await applyTemplate(record, helm, kubectl, "cilium", chartPins.cilium, "kube-system", fixturePaths.ciliumValues);
	await execute(kubectl, ["rollout", "status", "daemonset/cilium", "-n", "kube-system", "--timeout=240s"], env);
	await execute(kubectl, ["rollout", "status", "deployment/cilium-operator", "-n", "kube-system", "--timeout=240s"], env);
	await execute(kubectl, ["wait", "--for=condition=Ready", "node", "--all", "--timeout=240s"], env);
	await execute(kubectl, ["rollout", "status", "deployment/coredns", "-n", "kube-system", "--timeout=240s"], env);
	await applyFile(kubectl, fixturePaths.all, env);
	await applyFile(kubectl, await namespaceFile(record, "argocd"), env);
	const seedCommit = await seedRepository(record, kubectl, env);
	await applyFile(kubectl, await materialize(record, "git-server.yaml", fixturePaths.gitServer, seedCommit), env);
	await execute(kubectl, ["rollout", "status", "deployment/git-server", "-n", "e2e-system", "--timeout=120s"], env);
	await attempt(kubectl, ["delete", "pod", "git-probe", "-n", "argocd", "--ignore-not-found"], env);
	await execute(kubectl, ["run", "git-probe", "-n", "argocd", "--image=alpine/git:v2.54.0@sha256:03228d7ded483b0b3e55e1de7fa660d70d3e1bb432fa7021d29e192640de11aa", "--restart=Never", "--command", "--", "git", "ls-remote", gitRepositoryUrl], env);
	await execute(kubectl, ["wait", "--for=jsonpath={.status.phase}=Succeeded", "pod/git-probe", "-n", "argocd", "--timeout=120s"], env);
	await execute(kubectl, ["delete", "pod", "git-probe", "-n", "argocd", "--wait=true"], env);
	await applyTemplate(record, helm, kubectl, "argocd", chartPins.argocd, "argocd", fixturePaths.argoCdValues);
	for (const deployment of ["argocd-server", "argocd-repo-server", "argocd-applicationset-controller"]) await execute(kubectl, ["rollout", "status", `deployment/${deployment}`, "-n", "argocd", "--timeout=240s"], env);
	await execute(kubectl, ["rollout", "status", "statefulset/argocd-application-controller", "-n", "argocd", "--timeout=240s"], env);
	await applyFile(kubectl, await materialize(record, "root-application.yaml", fixturePaths.rootApplication), env);
	await waitApplication(kubectl, "root-application", env);
	for (const name of platformApplicationNames) await waitApplication(kubectl, name, env);
	for (const name of tenantApplicationNames) await waitApplication(kubectl, name, env);
	await execute(kubectl, ["rollout", "status", "deployment/metrics-server", "-n", "kube-system", "--timeout=180s"], env);
	await poll("metrics API", 180_000, async () => Boolean(await attempt(kubectl, ["get", "--raw", "/apis/metrics.k8s.io/v1beta1/nodes"], env)));
	await poll("HPA metrics", 180_000, async () => (await attempt(kubectl, ["get", "hpa", "catalog", "-n", "tenant-catalog", "-o", "jsonpath={.status.conditions[?(@.type=='ScalingActive')].status}"], env))?.stdout === "True");
	await execute(kubectl, ["wait", "--for=jsonpath={.status.phase}=Bound", "pvc/data-ledger-0", "-n", "tenant-ledger", "--timeout=180s"], env);
	await execute(kubectl, ["rollout", "status", "statefulset/ledger", "-n", "tenant-ledger", "--timeout=180s"], env);
	await execute(kubectl, ["wait", "--for=condition=Ready", "pod/declared-client", "pod/isolated-client", "-n", "tenant-catalog", "--timeout=120s"], env);
	await execute(kubectl, ["exec", "-n", "tenant-catalog", "declared-client", "--", "wget", "-qO-", "--timeout=3", "http://catalog"], env, true);
	if (await attempt(kubectl, ["exec", "-n", "tenant-catalog", "isolated-client", "--", "wget", "-qO-", "--timeout=3", "http://catalog"], env)) throw new Error("Cilium failed to deny the isolated client");
	await attempt(kubectl, ["delete", "pod", "ingress-probe", "-n", "tenant-catalog", "--ignore-not-found"], env);
	await execute(kubectl, ["run", "ingress-probe", "-n", "tenant-catalog", "--image=busybox:1.38.0@sha256:fd7dc98638c8e305f4dc34e979f1c0fdfdcaeb0fbf8fcff77ae834b6da3d7e6e", "--restart=Never", "--command", "--", "sh", "-ec", "wget -qO- --header='Host: catalog.e2e.local' http://traefik.traefik.svc.cluster.local | grep catalog-ingress-marker"], env);
	await execute(kubectl, ["wait", "--for=jsonpath={.status.phase}=Succeeded", "pod/ingress-probe", "-n", "tenant-catalog", "--timeout=120s"], env);
	await execute(kubectl, ["delete", "pod", "ingress-probe", "-n", "tenant-catalog", "--wait=true"], env);
	await execute(helm, ["upgrade", "--install", "operations", fixturePaths.chart, "-n", "operations", "--create-namespace"], env);
	await execute(kubectl, ["rollout", "status", "deployment/operations", "-n", "operations", "--timeout=120s"], env);
	await poll("expected CrashLoop incident", 120_000, async () => {
		const restarts = await attempt(kubectl, ["get", "pods", "-n", "operations", "-l", "app.kubernetes.io/name=operations-crashloop", "-o", "jsonpath={.items[0].status.containerStatuses[0].restartCount}"], env);
		const backoff = await attempt(kubectl, ["get", "events", "-n", "operations", "--field-selector=reason=BackOff", "-o", "name"], env);
		const logs = await attempt(kubectl, ["logs", "-n", "operations", "-l", "app.kubernetes.io/name=operations-crashloop", "--tail=5"], env);
		return Number(restarts?.stdout ?? "0") >= 2 && Boolean(backoff?.stdout.trim()) && logs?.stdout.includes("kubecove-crashloop-marker") === true;
	});
}

let current: Ownership | undefined;
async function create(kindName: Ownership["kind"]) {
	const dir = kindName === "run" ? runDir() : devDir; const id = kindName === "run" ? runId : workspaceHash; const file = recordPath(dir);
	if (kindName === "dev" && existsSync(file)) { const owned = await readOwnership(file, "dev", dir, id); const selected = await ensureProvider(owned.provider); const { kind, kubectl } = await tools(); if (await clusterExists(kind, owned.cluster, selected)) { current = owned; await bootstrap(owned, kubectl); await applyLab(owned); return owned; } await rm(dir, { recursive: true, force: true }); }
	if (existsSync(file)) throw new Error(`run ${runId} already exists`);
	const selected = await selectProvider(); const { kind, kubectl } = await tools(); const record: Ownership = { kind: kindName, runId: id, cluster: expectedCluster(kindName, id, workspaceHash), dir, raw: join(dir, "kind.raw.kubeconfig"), kubeconfig: join(dir, "kubeconfig"), dataDir: join(dir, "data"), kindConfig: join(dir, "kind.yaml"), disableDefaultCNI: true, provider: basename(selected.binary).startsWith("podman") ? "podman" : "docker", kubernetes };
	assertOwned(record, kindName, dir, id, workspaceHash); await mkdir(record.dataDir, { recursive: true }); await writeFile(record.kindConfig, kindConfig()); await writeFile(file, stringify(record)); await assertOwnedOnDisk(record, kindName, dir, id, workspaceHash); current = record;
	try { await execute(kind, ["create", "cluster", "--name", record.cluster, "--image", images[kubernetes], "--config", record.kindConfig, "--kubeconfig", record.raw], selected.env); await bootstrap(record, kubectl); await applyLab(record); return record; } catch (error) { if (kindName === "dev") { await stoppingChildren; await removeCluster(record); current = undefined; } throw error; }
}

async function safeArtifact(path: string, work: () => Promise<{ stdout: string; stderr: string } | undefined>) { try { const result = await work(); await writeFile(path, `${result?.stdout ?? ""}${result?.stderr ?? ""}`); } catch (error) { await writeFile(path, `diagnostic unavailable: ${String(error)}\n`); } }
async function diagnostics(record: Ownership) {
	await assertOwnedOnDisk(record, record.kind, record.dir, record.runId, workspaceHash);
	if (!existsSync(record.kubeconfig)) return;
	const artifacts = join(root, "e2e", "artifacts", record.runId); await mkdir(artifacts, { recursive: true }); const { kind, kubectl, helm } = await tools(); const env = { KUBECONFIG: record.kubeconfig };
	for (const [index, command] of safeDiagnosticCommands.entries()) await safeArtifact(join(artifacts, `status-${index}.txt`), async () => {
		const result = await attempt(kubectl, [...command], env);
		return result && { ...result, stdout: safeDiagnosticText(result.stdout), stderr: safeDiagnosticText(result.stderr) };
	});
	await safeArtifact(join(artifacts, "helm-releases.json"), async () => {
		const result = await attempt(helm, ["list", "--all-namespaces", "--output", "json"], env);
		return result && { ...result, stdout: safeDiagnosticText(result.stdout), stderr: safeDiagnosticText(result.stderr) };
	});
	const bytes = new Uint8Array(await readFile(record.kubeconfig)); const config = parseKubeconfig(new TextDecoder().decode(bytes));
	if (!config.contexts || !config.clusters?.[0]?.cluster.server) throw new Error("generated kubeconfig summary is incomplete");
	await writeFile(join(artifacts, "kubeconfig-summary.json"), JSON.stringify({ contexts: config.contexts.map(({ name }) => name), endpoint: new URL(config.clusters[0].cluster.server).host, loopback: true, sha256: sha256(bytes) }, null, 2));
	if (existsSync(join(record.dir, "seed-commit.txt"))) await writeFile(join(artifacts, "seed-commit.txt"), await readFile(join(record.dir, "seed-commit.txt")));
	const versions = await Promise.all([attempt(kind, ["version"]), attempt(kubectl, ["version", "--client", "-o", "yaml"]), attempt(helm, ["version", "--short"])]); await writeFile(join(artifacts, "versions.txt"), `${versions.map((value) => value?.stdout ?? "unavailable").join("\n")}\nimage ${imageFor(record.kubernetes)}\n`);
	await writeFile(join(artifacts, "verified-assets.json"), JSON.stringify({ ...Object.fromEntries(Object.entries(chartPins).map(([name, pin]) => [name, { version: pin.version, appVersion: "appVersion" in pin ? pin.appVersion : undefined, sha256: pin.sha256 }])), gitDaemon: { version: gitDaemonPins[cpu].version, architecture: cpu, sha256: gitDaemonPins[cpu].sha256 } }, null, 2));
}

function interrupt(signal: InterruptSignal) {
	if (interruptedBy) return;
	interruptedBy = signal;
	cancellation.abort(new Error(`Interrupted by ${signal}`));
	stoppingChildren = Promise.allSettled([...children].map((stop) => stop(signal))).then((results) => {
		for (const result of results) if (result.status === "rejected") console.error("Could not stop E2E child", result.reason);
	});
}

async function fast() {
	const artifacts = join(root, "e2e", "artifacts", `fast-${runId}`);
	await mkdir(dirname(artifacts), { recursive: true });
	await mkdir(artifacts);
	await writeFile(join(artifacts, "result.json"), JSON.stringify({ passed: false, status: "started" }, null, 2));
	try {
		if (!(await Array.fromAsync(new Bun.Glob("e2e/specs/fast/**/*.e2e.ts").scan({ cwd: root }))).length) throw new Error("fast suite has no specs");
		const { createServer, loadConfigFromFile } = await import("vite");
		const runDriver = async (name: "obscura" | "wdio") => {
			cancellation.signal.throwIfAborted();
			// Keep Chrome's socket path short and leave WDIO's temp-based download cache shared.
			const temporary = await mkdtemp(join(tmpdir(), `kc-e2e-${name}-`));
			const output = join(artifacts, name);
			const profile = join(temporary, "profile");
			let frontend: ViteDevServer | undefined;
			let http: ReturnType<typeof createHttpServer> | undefined;
			try {
				await mkdir(output);
				await mkdir(profile);
				const loaded = await loadConfigFromFile({ command: "serve", mode: "development" }, undefined, root);
				if (!loaded) throw new Error("Fast E2E needs the KubeCove Vite configuration");
				const config = loaded.config;
				frontend = await createServer({
					...config, configFile: false, root, cacheDir: join(temporary, "vite"),
					// The optional dev recorder writes to a shared artifacts directory.
					plugins: config.plugins?.filter((plugin) => !(plugin && "name" in plugin && plugin.name === "kubecove-dev-interactions")),
					define: { ...config.define, "process.env.KUBECOVE_PUBLIC_DEV_INTERACTIONS": JSON.stringify("") },
					// In middleware mode, Vite leaves signal handling and server shutdown to us.
					server: { ...config.server, middlewareMode: true, hmr: false, ws: false },
				});
				cancellation.signal.throwIfAborted();
				http = createHttpServer(frontend.middlewares);
				await once(http.listen(0, "127.0.0.1"), "listening");
				const address = http.address();
				if (!(address instanceof Object) || !("port" in address)) throw new Error("Vite did not bind a TCP port");
				cancellation.signal.throwIfAborted();
				const env = { E2E_FAST_URL: `http://127.0.0.1:${address.port}`, KUBECOVE_E2E_ARTIFACTS: output };
				if (name === "obscura") await runObscuraFast(env, profile);
				else {
					const report = join(output, "wdio-report.txt");
					await runWdio("e2e/wdio.fast.conf.ts", { ...env, E2E_FAST_CHROME_PROFILES: profile }, report).catch(async (error) => {
						const log = await readFile(report, "utf8").catch(() => null);
						if (log) console.error(log);
						throw error;
					});
				}
			} finally {
				try {
					const server = http;
					const servers = await Promise.allSettled([
						frontend?.close(),
						server?.listening && new Promise<void>((resolve, reject) => {
							server.close((error) => error ? reject(error) : resolve());
							server.closeAllConnections();
						}),
					]);
					rejectFailedPhases(servers, ["Vite", "HTTP server"]);
				}
				finally {
					// Chrome can outlive WDIO's launcher and still write to its profile.
					await stoppingChildren;
					await rm(temporary, { recursive: true, force: true });
				}
			}
		};
		const results = await Promise.allSettled([runDriver("obscura"), runDriver("wdio")]);
		rejectFailedPhases(results, ["Obscura", "WDIO"]);
		cancellation.signal.throwIfAborted();
		await writeFile(join(artifacts, "result.json"), JSON.stringify({ passed: true }, null, 2));
	} catch (error) {
		const result = interruptedBy ? { passed: false, status: "interrupted", signal: interruptedBy } : { passed: false, error: String(error) };
		await writeFile(join(artifacts, "result.json"), JSON.stringify(result, null, 2));
		throw error;
	}
}
function rejectFailedPhases(results: readonly PromiseSettledResult<unknown>[], names: readonly string[]): void {
	const failures = results.flatMap((result, index) => result.status === "rejected" ? [new Error(`${names[index]}: ${String(result.reason)}`, { cause: result.reason })] : []);
	if (failures.length) throw new AggregateError(failures, failures.map(({ message }) => message).join("\n"));
}
async function real() {
	if (keep && process.env.CI) throw new Error("--keep is forbidden in CI");
	const artifacts = join(root, "e2e", "artifacts", runId);
	// Both branches must finish before WDIO starts or teardown can touch the lab.
	const [lab, build] = await Promise.allSettled([create("run"), buildBrowserServer(root, artifacts, profiling, execute)]);
	try {
		rejectFailedPhases([lab, build], ["Kind lab", "Application build"]);
		if (lab.status !== "fulfilled" || build.status !== "fulfilled") throw new Error("Real E2E prerequisites did not finish");
		const record = lab.value;
		const env = {
			KUBECOVE_E2E_CLUSTER: record.cluster, KUBECOVE_E2E_KUBECTL: (await tools()).kubectl,
		};
		await runServerWdio(build.value, record.dataDir, record.kubeconfig, env, artifacts, cancellation.signal, runWdio);
	} finally {
		if (current) {
			if (!interruptedBy) await diagnostics(current).catch((failure) => console.error("diagnostics failed", failure));
			await stoppingChildren;
			if (!keep || (lab.status === "rejected" && !interruptedBy)) await removeCluster(current);
			current = undefined;
		}
	}
}
async function desktopSmoke() {
	const dir = join(stateDir, "desktop-smoke", runId);
	const artifacts = join(root, "e2e/artifacts", `desktop-${runId}`);
	const cluster = `kubecove-e2e-smoke-${runId}`;
	await mkdir(dirname(dir), { recursive: true });
	await mkdir(dir, { mode: 0o700 });
	let fixture: Awaited<ReturnType<typeof startMemoryFixture>> | undefined;
	try {
		await mkdir(dirname(artifacts), { recursive: true });
		await mkdir(artifacts);
		const dataDir = join(dir, "data");
		await mkdir(dataDir, { mode: 0o700 });
		if (memoryFixture === "normal" || memoryFixture === "large") fixture = await startMemoryFixture(memoryFixture, dir);
		const kubeconfig = await writeSmokeConfig(dir, cluster, fixture);
		const env = { KUBECOVE_E2E_CLUSTER: cluster, KUBECOVE_E2E_ARTIFACTS: artifacts, KUBECOVE_DATA_DIR: dataDir };
		const binary = await buildBrowserServer(root, artifacts, profiling, execute);
		if (profiling) {
			const sha = (await execute("git", ["rev-parse", "HEAD"], {}, true)).stdout.trim();
			const dirty = (await execute("git", ["status", "--porcelain"], {}, true)).stdout.trim();
			await runServerWdio(binary, dataDir, kubeconfig, { ...env, KUBECOVE_PROFILE: "1", KUBECOVE_PROFILE_SHA: sha,
				KUBECOVE_PROFILE_DIRTY: String(Boolean(dirty)), KUBECOVE_MEMORY_FIXTURE: memoryFixture,
				KUBECOVE_MEMORY_FIXTURE_ROWS: fixture && String(fixture.rows) }, artifacts, cancellation.signal, runWdio);
		} else {
			const target = join(root, "src-tauri/gpui/target");
			const buildEnv = { CARGO_TARGET_DIR: target, CARGO_BUILD_BUILD_DIR: join(root, "src-tauri/gpui/build"), RUSTC_WRAPPER: "" };
			const cargo = ["cargo", "build", "--locked", "--manifest-path", "src-tauri/gpui/Cargo.toml", "--features", "e2e"];
			if (os === "linux") {
				await mkdir(target, { recursive: true });
				await execute("flock", [join(target, "heavy.lock"), "nice", "-n", "10", ...cargo], buildEnv);
			} else await execute(cargo[0], cargo.slice(1), buildEnv);
			await execute("bun", ["e2e/gpui-smoke.ts"], { ...env, KUBECOVE_SERVER_BINARY: binary,
				KUBECOVE_GPUI_BINARY: join(target, "debug", `kubecove-gpui${suffix}`), KUBECOVE_KUBECONFIG: kubeconfig,
				KUBECOVE_E2E: "1", KUBECOVE_AUTOMATION_PORT: "0", HTTPS_PROXY: undefined, https_proxy: undefined });
		}
	} finally {
		await stoppingChildren;
		fixture?.stop();
		await rm(dir, { recursive: true, force: true });
	}
}
async function dev() {
	const record = await create("dev");
	try {
		const binary = await buildBrowserServer(root, join(root, "e2e/artifacts", `dev-${runId}`), profiling, execute);
		await runDevBrowser(binary, record.dataDir, record.kubeconfig, cancellation.signal);
	} finally { await stoppingChildren; current = undefined; }
}

const onInterrupt = () => interrupt("SIGINT");
const onTerminate = () => interrupt("SIGTERM");
if (["fast", "run", "dev-up", "desktop-smoke", "desktop-profile"].includes(action)) {
	process.on("SIGINT", onInterrupt);
	process.on("SIGTERM", onTerminate);
}
try {
	if (action === "fast") await fast();
	else if (action === "run") await real();
	else if (action === "desktop-smoke" || action === "desktop-profile") await desktopSmoke();
	else if (action === "cleanup") { if (!requestedRunId) throw new Error("cleanup requires --run-id <id>"); const dir = runDir(requestedRunId); if (existsSync(recordPath(dir))) await removeCluster(await readOwnership(recordPath(dir), "run", dir, requestedRunId)); }
	else if (action === "dev-up") await dev();
	else if (action === "dev-down") { if (existsSync(recordPath(devDir))) await removeCluster(await readOwnership(recordPath(devDir), "dev", devDir, workspaceHash, false)); }
	else throw new Error("use fast, run, desktop-smoke, desktop-profile, cleanup, dev-up, or dev-down");
} catch (error) {
	if (!interruptedBy) throw error;
	console.error(error);
} finally {
	await stoppingChildren;
	if (interruptedBy) {
		process.exitCode = interruptedBy === "SIGINT" ? 130 : 143;
		if (current?.kind === "dev") await rm(current.dataDir, { recursive: true, force: true });
	}
	process.off("SIGINT", onInterrupt);
	process.off("SIGTERM", onTerminate);
}
