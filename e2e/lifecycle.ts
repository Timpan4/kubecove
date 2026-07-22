import { createHash, randomUUID } from "node:crypto";
import { existsSync } from "node:fs";
import { chmod, mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { arch, platform } from "node:os";
import { basename, delimiter, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { parse, stringify } from "yaml";

const root = resolve(process.cwd());
const stateDir = join(root, ".e2e");
const args = Bun.argv.slice(3);
const option = (name: string) => {
	const index = args.indexOf(name);
	return index >= 0 ? args[index + 1] : undefined;
};
const action = Bun.argv[2];
const keep = args.includes("--keep");
const requestedRunId = option("--run-id") ?? process.env.E2E_RUN_ID;
const generatedRunId = `${Date.now()}-${process.pid}-${randomUUID().slice(0, 8)}`;
const runId = requestedRunId ?? generatedRunId;
const kubernetes = option("--kubernetes") ?? "1.35";
const requestedProvider = option("--provider") ?? "auto";
const workspaceHash = createHash("sha256").update(root).digest("hex").slice(0, 12);
const os = platform() === "darwin" ? "darwin" : platform() === "win32" ? "windows" : "linux";
const cpu = arch() === "arm64" ? "arm64" : "amd64";
const suffix = os === "windows" ? ".exe" : "";

if (!/^[a-zA-Z0-9][a-zA-Z0-9-]{0,47}$/.test(runId)) throw new Error("run ID must be 1-48 letters, digits, or hyphens");
if (!(["1.34", "1.35", "1.36"] as const).includes(kubernetes as "1.34")) throw new Error("--kubernetes must be 1.34, 1.35, or 1.36");

const images: Record<string, string> = {
	"1.34": "kindest/node:v1.34.8@sha256:02722c2dedddcfc00febf5d27fbeb9b7b2c14294c82109ff4a85d89ac9ba3256",
	"1.35": "kindest/node:v1.35.5@sha256:ce977ae6d65918d0b58a5f8b5e940429c2ce42fa3a5619ec2bbc60b949c0ac95",
	"1.36": "kindest/node:v1.36.1@sha256:3489c7674813ba5d8b1a9977baea8a6e553784dab7b84759d1014dbd78f7ebd5",
};
const pins = {
	kind: { "darwin-amd64": "295ac6d0d634c9819c9907df45e3017d1f13166bd13c3404c45e79f7faa47498", "darwin-arm64": "dca67911095a110c2b5c36e26df6cac860c602033e456c0db47be498cdef1ebb", "linux-amd64": "50030de23cf40a18505f20426f6a8506bedf13c6e509244bd1fa9463721b0f54", "linux-arm64": "b92cd615e97585de8ddade28ed5cd7feb4248d717c233eea5b03c37298900f5d", "windows-amd64": "0bcb2d1cfedc1912d664014db716937e8a0e843e91c6807b4db2025dbc8989fa" },
	kubectl: { "darwin-amd64": "fa6b472ca1e542e171d7daedd9435b8e9650bc18d42a57eb930a51f48ca58874", "darwin-arm64": "1827b555615791c1c1065dd64870eb49a4e00e9dfd389a82a2ef1d31bb46d200", "linux-amd64": "5d11e2ba01ea68ffd053f56e27738e2b4330013ee67f7e46c6da6c585d3c9926", "linux-arm64": "c0f97f31c9ddc22d4951d543a1a7125a9af4b31e895ad4aa99899c4ba2a6ff0b", "windows-amd64": "ce1c21d0e0a64fa249fad3b6372c14479e7901fedf6d5a425d92c46c2bd87442" },
	helm: { "darwin-amd64": "1376ea697140e4db316736e760d5a47d12afc1524dce704476ef06fd7fdeddc6", "darwin-arm64": "f13f959015447b6bc309f9fd506509926543988a39035c088b52522ec95e2acb", "linux-amd64": "97dbeb971be4ac4b27e3839976d9564c0fb35c6f3b1da89dd1e292d236af4096", "linux-arm64": "1f8de130dfbd04de64978e7b852a7a547be1404956a366608276d2520b678670", "windows-amd64": "614f68ddc567ac9bfb0c205f869b1f83ba4e0a9aacd26cbae47743ae6082a579" },
} as const;

type Provider = "docker" | "podman";
type Ownership = { kind: "run" | "dev"; runId: string; cluster: string; dir: string; raw: string; kubeconfig: string; dataDir: string; provider: Provider; kubernetes: string };
const runDir = (id = runId) => join(stateDir, "runs", id);
const devDir = join(stateDir, "dev", workspaceHash);
const recordPath = (dir: string) => join(dir, "ownership.json");
const key = () => `${os}-${cpu}` as keyof typeof pins.kind;
const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const contained = (path: string, parent: string) => { const value = relative(parent, path); return value === "" || (!value.startsWith("..") && !isAbsolute(value)); };

const children = new Set<ReturnType<typeof Bun.spawn>>();
function cleanEnvironment(extra: Record<string, string | undefined> = {}) {
	const env = { ...process.env };
	delete env.KUBECONFIG;
	for (const [name, value] of Object.entries(extra)) {
		if (value === undefined) delete env[name];
		else env[name] = value;
	}
	return env;
}
async function execute(cmd: string, commandArgs: string[], extra: Record<string, string | undefined> = {}, capture = false, logFile?: string) {
	const child = Bun.spawn([cmd, ...commandArgs], { cwd: root, env: cleanEnvironment(extra), stdout: capture || logFile ? "pipe" : "inherit", stderr: capture || logFile ? "pipe" : "inherit" });
	children.add(child);
	const [code, stdout, stderr] = await Promise.all([child.exited, child.stdout ? new Response(child.stdout).text() : "", child.stderr ? new Response(child.stderr).text() : ""]);
	children.delete(child);
	if (logFile) { await mkdir(dirname(logFile), { recursive: true }); await writeFile(logFile, `${stdout}${stderr}`); }
	if (code !== 0) throw new Error(`${basename(cmd)} failed (${code})`);
	return { stdout, stderr };
}
async function attempt(cmd: string, commandArgs: string[], extra: Record<string, string | undefined> = {}) {
	try { return await execute(cmd, commandArgs, extra, true); } catch { return undefined; }
}
async function runWdio(config: string, extra: Record<string, string | undefined>, logFile?: string) {
	const cli = join(root, "node_modules", "@wdio", "cli", "bin", "wdio.js");
	const version = await execute("node", ["--version"], {}, true);
	const match = /^v(\d+)\.(\d+)\.(\d+)/.exec(version.stdout.trim());
	if (!match) throw new Error(`could not parse Node version ${version.stdout.trim()}`);
	const [major, minor] = match.slice(1, 3).map(Number);
	if ((major === 18 && minor >= 20) || (major > 18 && major <= 24)) return execute("node", [cli, "run", config], extra, false, logFile);
	if (major < 18 || (major === 18 && minor < 20)) throw new Error(`WDIO requires Node 18.20-24; found ${version.stdout.trim()}`);
	const fnm = Bun.which("fnm");
	if (!fnm) throw new Error(`WDIO requires Node 18-24; found ${version.stdout.trim()} and no fnm fallback`);
	return execute(fnm, ["exec", "--using", "24", "node", cli, "run", config], extra, false, logFile);
}
async function verified(name: string, url: string, expected: string) {
	const target = join(stateDir, "tools", name);
	await mkdir(dirname(target), { recursive: true });
	if (existsSync(target) && hash(new Uint8Array(await readFile(target))) === expected) return target;
	await rm(target, { force: true });
	const response = await fetch(url);
	if (!response.ok) throw new Error(`download failed: ${name}`);
	const bytes = new Uint8Array(await response.arrayBuffer());
	if (hash(bytes) !== expected) throw new Error(`checksum mismatch: ${name}`);
	const temporary = `${target}.${process.pid}.part`;
	await writeFile(temporary, bytes); await rename(temporary, target);
	if (os !== "windows") await chmod(target, 0o755);
	return target;
}
let toolPromise: Promise<{ kind: string; kubectl: string; helm: string }> | undefined;
function tools() {
	if (toolPromise) return toolPromise;
	toolPromise = (async () => {
	if (!pins.kind[key()] || !pins.kubectl[key()] || !pins.helm[key()]) throw new Error(`unsupported tool platform ${key()}`);
	const kind = await verified(`kind${suffix}`, `https://github.com/kubernetes-sigs/kind/releases/download/v0.32.0/kind-${os}-${cpu}`, pins.kind[key()]);
	const kubectl = await verified(`kubectl${suffix}`, `https://dl.k8s.io/release/v1.35.6/bin/${os}/${cpu}/kubectl${suffix}`, pins.kubectl[key()]);
	const archiveName = `helm-v4.2.0-${os}-${cpu}.${os === "windows" ? "zip" : "tar.gz"}`;
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
	if (!(["auto", "docker", "podman"] as const).includes(requestedProvider as "auto")) throw new Error("--provider must be auto, docker, or podman");
	if (requestedProvider !== "auto") return ensureProvider(requestedProvider as Provider);
	for (const candidate of ["docker", "podman"] as const) { if (!providerBinary(candidate)) continue; try { return await ensureProvider(candidate); } catch {} }
	throw new Error("no usable Docker or Podman provider found");
}

function expectedCluster(kind: Ownership["kind"], id: string) { return kind === "run" ? `kubecove-e2e-${id}` : `kubecove-dev-${workspaceHash}`; }
function assertOwned(record: Ownership, kind: Ownership["kind"], dir: string, id: string) {
	if (record.kind !== kind || record.runId !== id || record.cluster !== expectedCluster(kind, id) || record.dir !== dir || ![record.raw, record.kubeconfig, record.dataDir].every((path) => contained(path, dir))) throw new Error("refuse operation outside exact ownership record");
}
async function readOwnership(file: string, kind: Ownership["kind"], dir: string, id: string) { const record = parse(await readFile(file, "utf8")) as Ownership; assertOwned(record, kind, dir, id); return record; }
async function clusterExists(kind: string, cluster: string, provider: Awaited<ReturnType<typeof ensureProvider>>) { const result = await attempt(kind, ["get", "clusters"], provider.env); return result?.stdout.split(/\r?\n/).includes(cluster) ?? false; }
async function removeCluster(record: Ownership) {
	const dir = record.kind === "run" ? runDir(record.runId) : devDir; assertOwned(record, record.kind, dir, record.runId);
	const { kind } = await tools(); const provider = await ensureProvider(record.provider);
	if (await clusterExists(kind, record.cluster, provider)) await execute(kind, ["delete", "cluster", "--name", record.cluster], provider.env);
	await rm(dir, { recursive: true, force: true });
}

function finalKubeconfig(raw: unknown, record: Ownership, token: string) {
	const config = raw as { apiVersion?: string; clusters?: Array<{ cluster: unknown }>; users?: Array<{ user: unknown }> };
	if (config.clusters?.length !== 1 || config.users?.length !== 1) throw new Error("unexpected Kind kubeconfig shape");
	return { apiVersion: config.apiVersion ?? "v1", kind: "Config", clusters: [{ name: record.cluster, cluster: config.clusters[0].cluster }], contexts: ["admin", "restricted"].map((role) => ({ name: `${record.cluster}-${role}`, context: { cluster: record.cluster, user: `${record.cluster}-${role}` } })), "current-context": `${record.cluster}-admin`, users: [{ name: `${record.cluster}-admin`, user: config.users[0].user }, { name: `${record.cluster}-restricted`, user: { token } }] };
}
function validateGeneratedConfig(value: unknown, record: Ownership) {
	const config = value as { clusters?: Array<{ name: string; cluster: { server?: string } }>; contexts?: Array<{ name: string; context: { cluster: string } }> };
	const contexts = config.contexts?.map(({ name }) => name).sort() ?? [];
	const expected = [`${record.cluster}-admin`, `${record.cluster}-restricted`].sort();
	if (JSON.stringify(contexts) !== JSON.stringify(expected) || config.clusters?.length !== 1 || config.clusters[0].name !== record.cluster || config.contexts?.some(({ context }) => context.cluster !== record.cluster)) throw new Error("generated kubeconfig scope mismatch");
	const server = new URL(config.clusters[0].cluster.server ?? ""); if (!["127.0.0.1", "localhost", "::1"].includes(server.hostname)) throw new Error("generated API server is not loopback");
}
async function bootstrap(record: Ownership, kubectl: string) {
	const rawEnv = { KUBECONFIG: record.raw };
	await execute(kubectl, ["apply", "-f", "e2e/fixtures/rbac.yaml"], rawEnv);
	const token = (await execute(kubectl, ["create", "token", "restricted", "-n", "e2e-system", "--duration=1h"], rawEnv, true)).stdout.trim();
	const config = finalKubeconfig(parse(await readFile(record.raw, "utf8")), record, token); validateGeneratedConfig(config, record);
	await writeFile(record.kubeconfig, stringify(config)); if (os !== "windows") await chmod(record.kubeconfig, 0o600);
}
async function applyLab(record: Ownership) {
	const { kubectl, helm } = await tools(); const env = { KUBECONFIG: record.kubeconfig };
	await execute(kubectl, ["apply", "-f", "e2e/fixtures/argocd-crd.yaml"], env);
	for (const name of ["applications.argoproj.io", "applicationsets.argoproj.io", "appprojects.argoproj.io"]) await execute(kubectl, ["wait", "--for=condition=Established", `crd/${name}`, "--timeout=120s"], env);
	await execute(kubectl, ["apply", "-f", "e2e/fixtures/all.yaml"], env);
	await execute(kubectl, ["rollout", "status", "deployment/fixture-api", "-n", "e2e-sessions", "--timeout=120s"], env);
	await execute(helm, ["upgrade", "--install", "fixture-chart", "e2e/fixtures/chart", "-n", "e2e-integrations", "--create-namespace", "--wait", "--timeout", "2m"], env);
}

let current: Ownership | undefined;
async function create(kindName: Ownership["kind"]) {
	const dir = kindName === "run" ? runDir() : devDir; const id = kindName === "run" ? runId : workspaceHash; const file = recordPath(dir);
	if (kindName === "dev" && existsSync(file)) { const owned = await readOwnership(file, "dev", dir, id); const selected = await ensureProvider(owned.provider); const { kind, kubectl } = await tools(); if (await clusterExists(kind, owned.cluster, selected)) { current = owned; await bootstrap(owned, kubectl); await applyLab(owned); return owned; } await rm(dir, { recursive: true, force: true }); }
	if (existsSync(file)) throw new Error(`run ${runId} already exists`);
	const selected = await selectProvider(); const { kind, kubectl } = await tools(); const record: Ownership = { kind: kindName, runId: id, cluster: expectedCluster(kindName, id), dir, raw: join(dir, "kind.raw.kubeconfig"), kubeconfig: join(dir, "kubeconfig"), dataDir: join(dir, "data"), provider: basename(selected.binary).startsWith("podman") ? "podman" : "docker", kubernetes };
	await mkdir(record.dataDir, { recursive: true }); await execute(kind, ["create", "cluster", "--name", record.cluster, "--image", images[kubernetes], "--kubeconfig", record.raw], selected.env);
	await writeFile(file, stringify(record)); current = record;
	try { await bootstrap(record, kubectl); await applyLab(record); return record; } catch (error) { if (kindName === "run") await diagnostics(record).catch((failure) => console.error("diagnostics failed", failure)); await removeCluster(record); current = undefined; throw error; }
}

async function safeArtifact(path: string, work: () => Promise<{ stdout: string; stderr: string } | undefined>) { try { const result = await work(); await writeFile(path, `${result?.stdout ?? ""}${result?.stderr ?? ""}`); } catch (error) { await writeFile(path, `diagnostic unavailable: ${String(error)}\n`); } }
async function diagnostics(record: Ownership) {
	if (!existsSync(record.kubeconfig)) return;
	const artifacts = join(root, "e2e", "artifacts", record.runId); await mkdir(artifacts, { recursive: true }); const { kind, kubectl, helm } = await tools(); const env = { KUBECONFIG: record.kubeconfig };
	await safeArtifact(join(artifacts, "inventory.txt"), () => attempt(kubectl, ["get", "all,configmaps,namespaces,applications.argoproj.io,applicationsets.argoproj.io,appprojects.argoproj.io", "-A", "-o", "wide"], env));
	await safeArtifact(join(artifacts, "events.txt"), () => attempt(kubectl, ["get", "events", "-A"], env));
	await safeArtifact(join(artifacts, "fixture-pod.log"), () => attempt(kubectl, ["logs", "-n", "e2e-sessions", "deployment/fixture-api"], env));
	const provider = await ensureProvider(record.provider); await attempt(kind, ["export", "logs", join(artifacts, "kind"), "--name", record.cluster], provider.env);
	const bytes = new Uint8Array(await readFile(record.kubeconfig)); const config = parse(new TextDecoder().decode(bytes)) as { contexts: Array<{ name: string }>; clusters: Array<{ cluster: { server: string } }> };
	await writeFile(join(artifacts, "kubeconfig-summary.json"), JSON.stringify({ contexts: config.contexts.map(({ name }) => name), endpoint: new URL(config.clusters[0].cluster.server).host, loopback: true, sha256: hash(bytes) }, null, 2));
	const versions = await Promise.all([attempt(kind, ["version"]), attempt(kubectl, ["version", "--client", "-o", "yaml"]), attempt(helm, ["version", "--short"])]); await writeFile(join(artifacts, "versions.txt"), `${versions.map((value) => value?.stdout ?? "unavailable").join("\n")}\nimage ${images[record.kubernetes]}\n`);
}

let shuttingDown = false;
async function shutdown(signal: "SIGINT" | "SIGTERM") { if (shuttingDown) return; shuttingDown = true; for (const child of children) child.kill(signal); if (current?.kind === "run") { await diagnostics(current).catch((failure) => console.error("diagnostics failed", failure)); if (!keep) await removeCluster(current); } else if (current) await rm(current.dataDir, { recursive: true, force: true }); process.exit(signal === "SIGINT" ? 130 : 143); }
if (["run", "dev-up"].includes(action)) for (const signal of ["SIGINT", "SIGTERM"] as const) process.once(signal, () => void shutdown(signal));

async function fast() {
	if (!(await Array.fromAsync(new Bun.Glob("e2e/specs/fast/**/*.e2e.ts").scan({ cwd: root }))).length) throw new Error("fast suite has no specs");
	const vite = Bun.spawn(["bun", "run", "dev", "--host", "127.0.0.1", "--port", "1420", "--strictPort"], { cwd: root, stdout: "inherit", stderr: "inherit" });
	try { for (let count = 0; count < 80; count++) { try { if ((await fetch("http://127.0.0.1:1420")).ok) break; } catch {} if (count === 79) throw new Error("Vite did not become ready"); await Bun.sleep(250); } await runWdio("e2e/wdio.fast.conf.ts", { KUBECOVE_E2E_ARTIFACTS: join(root, "e2e", "artifacts", "fast") }); }
	finally { vite.kill(); await vite.exited; }
}
async function buildAndDrive(env: Record<string, string | undefined>, smoke = false) { const artifacts = env.KUBECOVE_E2E_ARTIFACTS as string; await execute("bun", ["run", "tauri", "build", "--debug", "--no-bundle", "--config", "src-tauri/tauri.e2e.conf.json", "--features", "e2e"], env, false, join(artifacts, "build.log")); await runWdio("e2e/wdio.real.conf.ts", { ...env, KUBECOVE_E2E_BINARY: join(root, "src-tauri", "target", "debug", `kubecove${suffix}`), KUBECOVE_E2E_SMOKE: smoke ? "1" : undefined }, join(artifacts, "wdio.log")); }
async function real() { if (keep && process.env.CI) throw new Error("--keep is forbidden in CI"); const record = await create("run"); const artifacts = join(root, "e2e", "artifacts", record.runId); const env = { KUBECOVE_E2E: "1", KUBECOVE_KUBECONFIG: record.kubeconfig, KUBECOVE_DATA_DIR: record.dataDir, KUBECOVE_E2E_CLUSTER: record.cluster, KUBECOVE_E2E_KUBECTL: (await tools()).kubectl, KUBECOVE_E2E_ARTIFACTS: artifacts }; try { await buildAndDrive(env); } finally { await diagnostics(record).catch((failure) => console.error("diagnostics failed", failure)); if (!keep) await removeCluster(record); current = undefined; } }
async function desktopSmoke() { const dir = join(stateDir, "desktop-smoke", runId); const cluster = `kubecove-e2e-smoke-${runId}`; const kubeconfig = join(dir, "kubeconfig"); const dataDir = join(dir, "data"); const artifacts = join(root, "e2e", "artifacts", `desktop-${runId}`); await mkdir(dataDir, { recursive: true }); const config = { apiVersion: "v1", kind: "Config", clusters: [{ name: cluster, cluster: { server: "https://127.0.0.1:65535" } }], contexts: ["admin", "restricted"].map((role) => ({ name: `${cluster}-${role}`, context: { cluster, user: `${cluster}-${role}` } })), "current-context": `${cluster}-admin`, users: ["admin", "restricted"].map((role) => ({ name: `${cluster}-${role}`, user: { token: "redacted-smoke-value" } })) }; await writeFile(kubeconfig, stringify(config)); const env = { KUBECOVE_E2E: "1", KUBECOVE_KUBECONFIG: kubeconfig, KUBECOVE_DATA_DIR: dataDir, KUBECOVE_E2E_CLUSTER: cluster, KUBECOVE_E2E_ARTIFACTS: artifacts }; try { await buildAndDrive(env, true); } finally { await rm(dir, { recursive: true, force: true }); } }
async function dev() { try { if ((await fetch("http://127.0.0.1:1430")).ok) throw new Error("an existing KubeCove dev server is running; dev:kind will not restart it"); } catch (error) { if (error instanceof Error && error.message.startsWith("an existing")) throw error; } const record = await create("dev"); await rm(record.dataDir, { recursive: true, force: true }); await mkdir(record.dataDir, { recursive: true }); try { await execute("bun", ["run", "tauri", "dev", "--config", "src-tauri/tauri.dev-kind.conf.json"], { KUBECONFIG: record.kubeconfig, KUBECOVE_DEV_KIND: "1", KUBECOVE_DATA_DIR: record.dataDir }); } finally { await rm(record.dataDir, { recursive: true, force: true }); current = undefined; } }

if (action === "fast") await fast();
else if (action === "run") await real();
else if (action === "desktop-smoke") await desktopSmoke();
else if (action === "cleanup") { if (!requestedRunId) throw new Error("cleanup requires --run-id <id>"); const dir = runDir(requestedRunId); if (existsSync(recordPath(dir))) await removeCluster(await readOwnership(recordPath(dir), "run", dir, requestedRunId)); }
else if (action === "dev-up") await dev();
else if (action === "dev-down") { if (existsSync(recordPath(devDir))) await removeCluster(await readOwnership(recordPath(devDir), "dev", devDir, workspaceHash)); }
else throw new Error("use fast, run, desktop-smoke, cleanup, dev-up, or dev-down");
