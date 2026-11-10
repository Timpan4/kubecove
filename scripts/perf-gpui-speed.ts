// Action-to-settled latency of the GPUI client on its real NativeBackend, against the loopback
// Kubernetes fixture with an optional per-response delay. Measurement only; Linux or macOS.
// Usage: bun scripts/perf-gpui-speed.ts --bin <release `speed` test binary>
//        [--fixture large|normal] [--latency-ms 0,25] [--out <dir>]
// Build the binary with `cargo test --release --features frame-profile --test speed --no-run`
// (see AGENTS.md "GPUI work" for the build environment and lock). Each latency writes
// speed-<ms>.json (the binary's report), requests-<ms>.json (every request the fixture served)
// and stderr-<ms>.log (the client's `[kubecove:http]` and `[kubecove:load]` lines).
import { mkdir, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { stringify } from "yaml";
import { MEMORY_FIXTURE_ROWS, type MemoryFixture, startMemoryFixture } from "../e2e/harness/memory-fixture";

function isMemoryFixture<Value>(value: Value): value is Value & MemoryFixture {
	return Object.hasOwn(MEMORY_FIXTURE_ROWS, String(value));
}

const args = process.argv.slice(2);
const option = (name: string, fallback: string) => {
	const index = args.indexOf(name);
	return index === -1 ? fallback : (args[index + 1] ?? fallback);
};
const binary = resolve(option("--bin", ""));
const fixtureOption = option("--fixture", "large");
if (!isMemoryFixture(fixtureOption)) throw new Error(`--fixture must name a memory fixture: ${fixtureOption}`);
const fixtureSize = fixtureOption;
const latencies = option("--latency-ms", "0,25").split(",").map(Number);
const out = resolve(option("--out", join("e2e", "artifacts", `gpui-speed-${new Date().toISOString().replace(/[:.]/g, "-")}`)));
if (!binary || !(await Bun.file(binary).exists())) throw new Error("--bin must name the built speed test binary");
await mkdir(out, { recursive: true });

type Action = { label: string; startEpochMs: number; endEpochMs: number };

for (const latencyMs of latencies) {
	const directory = join(out, `fixture-${latencyMs}`);
	// An isolated HOME keeps the client away from the user's kubeconfig and settings.
	const home = join(directory, "home");
	await mkdir(home, { recursive: true });
	const fixture = await startMemoryFixture(fixtureSize, directory, { latencyMs });
	const report = join(out, `speed-${latencyMs}.json`);
	// Both servers keep the runner alive, so they stop on every path out of this run.
	let control: ReturnType<typeof Bun.serve> | undefined;
	let code: number;
	try {
		control = Bun.serve({
			hostname: "127.0.0.1", port: 0,
			fetch(request) {
				const url = new URL(request.url);
				if (url.pathname !== "/burst") return new Response("not found", { status: 404 });
				const events = url.searchParams.get("events");
				return Response.json(fixture.burst(Number(url.searchParams.get("restarts")), events ? Number(events) : undefined));
			},
		});
		const cluster = "kubecove-speed";
		const kubeconfig = join(directory, "kubeconfig");
		await writeFile(kubeconfig, stringify({
			"apiVersion": "v1", "kind": "Config",
			"clusters": [{ name: cluster, cluster: { server: fixture.url, "certificate-authority-data": fixture.authority } }],
			"contexts": ["admin", "restricted"].map((role) => ({ name: `${cluster}-${role}`, context: { cluster, user: `${cluster}-${role}` } })),
			"current-context": `${cluster}-admin`,
			"users": ["admin", "restricted"].map((role) => ({ name: `${cluster}-${role}`, user: { token: "redacted-speed-value" } })),
		}));
		const child = Bun.spawn([binary], {
			env: {
				PATH: process.env.PATH ?? "",
				HOME: home,
				XDG_CONFIG_HOME: join(home, ".config"),
				XDG_DATA_HOME: join(home, ".local", "share"),
				KUBECONFIG: kubeconfig,
				KUBECOVE_SPEED_REPORT: report,
				KUBECOVE_SPEED_CONTROL: `127.0.0.1:${control.port}`,
				KUBECOVE_SPEED_CONTEXTS: `${cluster}-admin,${cluster}-restricted`,
				KUBECOVE_SPEED_LATENCY_MS: String(latencyMs),
				KUBECOVE_SPEED_ROWS: String(fixture.rows),
			},
			stdout: "inherit",
			stderr: Bun.file(join(out, `stderr-${latencyMs}.log`)),
		});
		code = await child.exited;
	} finally {
		control?.stop(true);
		fixture.stop();
	}
	await writeFile(join(out, `requests-${latencyMs}.json`), JSON.stringify(fixture.requests));
	if (code !== 0) throw new Error(`speed binary exited ${code}; see stderr-${latencyMs}.log`);
	const actions: Action[] = (await Bun.file(report).json()).actions;
	// Server-side reads per action, watches included, keyed by method and path without query.
	const perAction = actions.map((action) => {
		const served = fixture.requests.filter((request) => request.epochMs >= action.startEpochMs && request.epochMs <= action.endEpochMs);
		const paths: Record<string, number> = {};
		for (const request of served) {
			const key = `${request.watch ? "WATCH" : request.method} ${request.path.split("?")[0]}`;
			paths[key] = (paths[key] ?? 0) + 1;
		}
		return { label: action.label, served: served.length, watches: served.filter((request) => request.watch).length, paths };
	});
	await writeFile(join(out, `reads-${latencyMs}.json`), JSON.stringify(perAction, null, 1));
	console.log(`speed latency=${latencyMs}ms -> ${report}`);
}
