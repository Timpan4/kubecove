// Times the views and interactions listed in tests/frames/cases.rs and writes a
// frame-time report to e2e/artifacts/gpui-frames-<time>/.
// Usage: nice -n 10 bun scripts/gpui-frames.ts [--runs N] [--frames N] [--warmup N] [--no-gpu]
//        [--compare <report.json>] [--binary <frames test binary>] [--manifest <Cargo.toml>] [--fixture <fixture.json>] [case,case]
// `--binary` times a saved build of the harness, such as one from before a change.
// The harness is `src-tauri/gpui/tests/frames.rs`; its header says what each number includes.

import { createHash } from "node:crypto";
import { existsSync, realpathSync } from "node:fs";
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { cpus, loadavg } from "node:os";
import { runFrameCommand } from "./gpui-frame-process";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { VIEWS } from "../e2e/parity/visual/views";
import {
	aggregate,
	type Case,
    frameCounts,
	type Metric,
	type MetricStats,
	median,
	samplesOf,
	stats,
} from "./gpui-frame-stats";

/** The requested p95 ceiling for the 120 fps gate, in milliseconds. */
const BUDGET_MS = 8.33;

// `input` is the name earlier harness builds gave `step`; both time the same work.
type Run = {
	build?: { manifestDirectory: string;
        profiling?: boolean; debugAssertions?: boolean; targetDirectory: string; buildDirectory: string };
	measurementVersion?: number;
	mode?: string;
	window: { width: number; height: number };
	warmup: number;
	frames: number;
	sizes: Record<string, { count: number; source: string }>;
	gpuFloor: number[];
	cases: Case[];
};
type Summary = {
	id: string;
	kind: string;
	description: string;
	metrics: Partial<Record<Metric, MetricStats>>;
	changed: number;
	frames: number;
	coldDrawMs: number;
	framesToSettle?: MetricStats;
    gpuFramesToSettle?: MetricStats;
    gpuCompletionPresentations?: MetricStats;
	completionPresentations?: MetricStats;
	frameOrdinals: MetricStats[];
	gpuFrameOrdinals: MetricStats[];
};

function option(name: string): string | undefined {
	const index = process.argv.indexOf(name);
	return index === -1 ? undefined : process.argv[index + 1];
}
function count(name: string, fallback: number): number {
	const value = Number(option(name) ?? fallback);
	if (!Number.isInteger(value) || value < 1)
		throw new Error(`${name} must be a positive integer`);
	return value;
}
const flags = new Set([
	"--runs",
	"--frames",
	"--warmup",
	"--compare",
	"--binary",
	"--manifest",
	"--fixture",
]);
const positional = process.argv
	.slice(2)
	.filter(
		(arg, index, all) =>
			!arg.startsWith("--") && !flags.has(all[index - 1] ?? ""),
	);
// Three runs give a spread. Each frame ordinal has up to 240 measured samples,
// enough for nearest-rank p99, after 120 warm-up iterations.
const runs = count("--runs", 3);
const frames = count("--frames", 240);
const warmup = count("--warmup", 120);
const gpu = !process.argv.includes("--no-gpu");
const compare = option("--compare");
const binary = option("--binary");
const fixtureSource = option("--fixture");
const cases = positional.join(",");

const out = resolve(
	"e2e/artifacts",
	`gpui-frames-${new Date().toISOString().replace(/[:.]/g, "-")}`,
);
await mkdir(out, { recursive: true });

const run = (command: string[], env: Record<string, string | undefined>, log: string, guardParity = false) =>
	runFrameCommand(command, env, join(out, log), guardParity);

const fixture = join(out, "fixture.json");
if (fixtureSource) {
	await copyFile(resolve(fixtureSource), fixture);
} else {
	await run(
		["bun", "e2e/parity/visual/export-fixture.ts", fixture],
		process.env,
		"export.log",
	);
}

await run(
    ["bun", "e2e/parity/visual/export-watch-burst.ts", fixture, join(out, "fixture-watch-burst.json")],
    process.env,
    "export-watch-burst.log",
);
const workspace = resolve("src-tauri/gpui");
const manifest = realpathSync(option("--manifest") ?? join(workspace, "Cargo.toml"));
const sourceDirectory = dirname(manifest);
const sourceRelative = relative(realpathSync("."), sourceDirectory);
if (sourceRelative.split(sep)[0] === ".." || isAbsolute(sourceRelative)) {
	throw new Error("The source manifest must stay inside this worktree");
}
// One build job by default, as `gpui-visual.ts` does on a shared workstation.
const env = {
	...process.env,
	CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? "1",
	CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? join(workspace, "target"),
	CARGO_BUILD_BUILD_DIR:
		process.env.CARGO_BUILD_BUILD_DIR ?? join(workspace, "build"),
};
Reflect.deleteProperty(env, "ARGV0");
for (const [name, expected] of [
	["CARGO_TARGET_DIR", join(workspace, "target")],
	["CARGO_BUILD_BUILD_DIR", join(workspace, "build")],
] as const) {
	if (resolve(env[name] ?? "") !== expected) throw new Error(`${name} must use this worktree's directory: ${expected}`);
}
const cargo = [
	"cargo",
	"test",
	"--release",
	"--locked",
	"--manifest-path",
	manifest,
	"--test",
	"frames",
];
// Apply nice once when launching this runner; record the child's observed priority.
const timed = (command: string[]) =>
	process.platform === "linux" && existsSync("/usr/bin/time")
		? ["/usr/bin/time", "-v", ...command]
		: command;
const buildLog = binary
	? ""
	: (await run(timed([...cargo, "--no-run"]), env, "build.log")).stderr;
const peakKb = Number(
	/Maximum resident set size \(kbytes\): (\d+)/.exec(buildLog)?.[1] ??
		Number.NaN,
);
// Runs execute the built binary, so a source edit during a run cannot trigger a rebuild.
const builtExecutable = binary
	? resolve(binary)
	: /Executable tests[\\/]frames\.rs \((.+)\)/.exec(buildLog)?.[1];
if (!builtExecutable)
	throw new Error(`no frames executable in ${join(out, "build.log")}`);
// Pin the executable so later local builds cannot replace it between repetitions.
const executable = join(
	out,
	process.platform === "win32" ? "frames-executable.exe" : "frames-executable",
);
await copyFile(builtExecutable, executable);
const binarySha256 = createHash("sha256").update(await readFile(executable)).digest("hex");
const harness = [executable];

const reports: Run[] = [];
const loads: { before: number[]; after: number[]; priority: number | null; uptime: string | null }[] = [];
for (let index = 0; index < runs; index += 1) {
	const report = join(out, `run-${index + 1}.json`);
	let uptime: string | null = null;
	if (process.platform !== "win32") {
		const child = Bun.spawn(["uptime"], { stdout: "pipe", stderr: "pipe" });
		const [code, output, error] = await Promise.all([
			child.exited, new Response(child.stdout).text(), new Response(child.stderr).text(),
		]);
		if (code !== 0) throw new Error(`uptime exited ${code}: ${error}`);
		uptime = output.trim();
		console.log(`Run ${index + 1}: ${uptime}`);
	}
	const before = loadavg();
	const { priority } = await run(
		harness,
		{
			...env,
			KUBECOVE_FRAMES_FIXTURE: fixture,
			KUBECOVE_FRAMES_REPORT: report,
			KUBECOVE_FRAMES_WARMUP: String(warmup),
			KUBECOVE_FRAMES_COUNT: String(frames),
			KUBECOVE_FRAMES_GPU: gpu ? "1" : "0",
			KUBECOVE_FRAMES_CASES: cases,
		},
		`run-${index + 1}.log`,
		true,
	);
	loads.push({ before, after: loadavg(), priority, uptime });
	const measured: Run = JSON.parse(await readFile(report, "utf8"));
	if (measured.measurementVersion !== 2 || measured.mode !== "Production")
		throw new Error("Per-frame acceptance requires the production-mode version 2 harness");
	if (
		!measured.build ||
		resolve(measured.build.manifestDirectory) !== sourceDirectory ||
		resolve(measured.build.targetDirectory) !== join(workspace, "target") ||
		resolve(measured.build.buildDirectory) !== join(workspace, "build")
	) throw new Error("The executable must be compiled in this worktree's target and build directories");
	if (measured.build.profiling || measured.build.debugAssertions) throw new Error("Instrumented or debug builds cannot satisfy the frame gate");
	reports.push(measured);
}

const ids = reports[0].cases.map(entry => entry.id);
if (new Set(ids).size !== ids.length) throw new Error("Duplicate frame case IDs");
if (!cases) {
    const measuredViews = new Set(reports[0].cases.filter(entry => entry.kind === "view").map(entry => entry.id));
    const missing = VIEWS.filter(id => !measuredViews.has(id));
    if (missing.length) throw new Error(`Missing frame cases for parity views: ${missing.join(", ")}`);
}

const summaries: Summary[] = reports[0].cases.map((first) => {
	const perRun = reports.map((report) => {
		const entry = report.cases.find((entry) => entry.id === first.id);
		if (
			!entry ||
			entry.draw.length !== frames ||
			(entry.kind === "interaction" &&
				(entry.step ?? entry.input)?.length !== frames)
		)
			throw new Error(`Missing samples for ${first.id}`);
		if (
			report.measurementVersion === 2 &&
			(entry.cpuFrames?.length !== frames ||
				entry.cpuFrames.some(
					(frames) =>
						!frames.length ||
						frames.some(
							(frame) => !Number.isFinite(frame.cpu) || frame.cpu < frame.draw,
						),
				))
		)
			throw new Error(`Invalid CPU frames for ${first.id}`);
		if ((gpu || entry.gpu.length !== 0) && entry.gpu.length !== frames)
			throw new Error(`Incomplete offscreen samples for ${first.id}`);
		if (entry.kind === "interaction" && entry.gpuCpuFrames?.length && entry.gpuStep?.length !== frames)
			throw new Error(`Incomplete renderer settling samples for ${first.id}`);
		if (entry.kind === "interaction" && entry.changed !== frames)
			throw new Error(
				`Ineffective inputs in ${first.id}: ${entry.changed}/${frames}`,
			);
		return entry;
	});
	const metrics: Summary["metrics"] = {};
	for (const metric of [
		"frame",
		"draw",
		"step",
		"gpu",
		"inputWorst",
		"presentation",
		"gpuFrame",
		"gpuPresentation",
		"gpuWorst",
		"threadCpu",
		"runqueue",
		"gpuSubmit",
		"gpuThreadCpu",
		"gpuRunqueue",
        "gpuStep",
	] satisfies Metric[]) {
		const collected = perRun.map((entry) => samplesOf(entry, metric));
        const series = metric === "presentation" || metric === "gpuPresentation"
            ? collected.filter(samples => samples.length > 0) : collected;
		if (!series.length || series.some((samples) => samples.length === 0)) continue;
		metrics[metric] = aggregate(series);
	}
	const ordinalCount = Math.max(
		...perRun.flatMap(
			(entry) =>
				entry.cpuFrames?.map(
					(frames) => frames.filter((frame) => !frame.presentationOnly).length,
				) ?? [0],
		),
	);
	const frameOrdinals = Array.from({ length: ordinalCount }, (_, ordinal) =>
		aggregate(
			perRun
				.map(
					(entry) =>
						entry.cpuFrames?.flatMap((frames) => {
							const draws = frames.filter((frame) => !frame.presentationOnly);
							return draws[ordinal] ? [draws[ordinal].cpu] : [];
						}) ?? [],
				)
				.filter((samples) => samples.length > 0),
		),
	);
	const gpuOrdinalCount = Math.max(
		...perRun.flatMap(
			(entry) =>
				entry.gpuCpuFrames?.map(
					(frames) => frames.filter((frame) => !frame.presentationOnly).length,
				) ?? [0],
		),
	);
	const gpuFrameOrdinals = Array.from(
		{ length: gpuOrdinalCount },
		(_, ordinal) =>
			aggregate(
				perRun
					.map(
						(entry) =>
							entry.gpuCpuFrames?.flatMap((frames) => {
								const draws = frames.filter((frame) => !frame.presentationOnly);
								return draws[ordinal] ? [draws[ordinal].cpu] : [];
							}) ?? [],
					)
					.filter((samples) => samples.length > 0),
			),
	);
	const completionPresentations = frameCounts(perRun, "cpuFrames", true);
	const framesToSettle = frameCounts(perRun, "cpuFrames", false);
	const gpuFramesToSettle = frameCounts(perRun, "gpuCpuFrames", false);
	const gpuCompletionPresentations = frameCounts(perRun, "gpuCpuFrames", true);
	return {
		id: first.id,
		kind: first.kind,
		description: first.description,
		metrics,
		frameOrdinals,
		gpuFrameOrdinals,
		framesToSettle,
        gpuFramesToSettle,
        gpuCompletionPresentations,
		completionPresentations,
		changed: median(perRun.map((entry) => entry.changed)),
		frames: first.draw.length,
		coldDrawMs: median(perRun.map((entry) => entry.coldDraw)) / 1e6,
	};
});
const floor = reports.flatMap((report) => report.gpuFloor);
const gpuSizes = [
	...new Set(
		reports.flatMap((report) =>
			report.cases.flatMap((entry) =>
				entry.gpuSize ? [entry.gpuSize.join("x")] : [],
			),
		),
	),
];
const result = {
	build: reports[0].build,
	binarySha256,
	measurementVersion: reports[0].measurementVersion ?? 1,
	mode: reports[0].mode ?? "Test",
	budgetMs: BUDGET_MS,
	runs,
	frames,
	warmup,
	gpu,
	cpu: cpus()[0]?.model,
	cores: cpus().length,
	loads,
	buildPeakRssMb: peakKb / 1024,
	sizes: reports[0].sizes,
	window: reports[0].window,
	gpuSizes,
	gpuFloor: floor.length ? stats(floor) : null,
	summaries,
};
await writeFile(join(out, "report.json"), JSON.stringify(result, null, 2));

const overBudget = (summary: Summary) =>
	[
		summary.metrics.frame,
		summary.metrics.gpuFrame,
		summary.metrics.presentation,
		summary.metrics.gpuPresentation,
		summary.metrics.inputWorst,
		summary.metrics.gpuWorst,
		...summary.frameOrdinals,
		...summary.gpuFrameOrdinals,
	].some((metric) => metric && metric.p95Spread[1] > BUDGET_MS);
const ms = (value: number | undefined) =>
	value === undefined ? "-" : value.toFixed(2);
const lines = [
	"# GPUI frame timing",
	"",
	`Budget: p95 at most ${BUDGET_MS.toFixed(2)} ms per frame (120 fps). Window ${result.window.width}x${result.window.height} logical pixels, release profile, ${runs} runs of ${frames} measured samples after ${warmup} warm-up samples per case. An interaction sample includes one input and every frame it produces. Case percentiles are medians across runs; max is the largest sample in any run. The p95 spread is the lowest and highest run. CPU samples are wall time around CPU work, including waits and descheduling.`,
	"",
	`CPU: ${result.cpu} (${result.cores} threads). Load averages (1/5/15 min) before and after each run: ${loads.map((load) => `${load.before.map((value) => value.toFixed(2)).join("/")} -> ${load.after.map((value) => value.toFixed(2)).join("/")} (process priority ${load.priority ?? "unavailable"})`).join(", ")}. Release build peak RSS: ${Number.isFinite(result.buildPeakRssMb) ? `${ms(result.buildPeakRssMb)} MiB` : "not recorded"}.`,
	`Build provenance: ${JSON.stringify(result.build)}. Executable SHA-256: ${binarySha256}.`,
	"",
	result.measurementVersion === 2
		? "`frame` includes the input/state update and executor drain before a scheduled production-mode frame, plus that frame callback including render, layout, prepaint, scene paint and cleanup. Every draw has its own sample, including synchronous redraws inside native key dispatch. CPU intervals split at SDK draw completion, carrying subsequent update work into the next frame; callback cleanup is included and the final tail is charged to the last frame. Content-settled latency ends at the last content draw's callback completion. Remaining ready work without a draw is conservatively charged to the last CPU sample, so frame CPU can slightly exceed content-settled latency. Presentation-only VRR keepalive callbacks have separate samples and stay in the CPU gate; they do not extend content-settled latency. Future timers are not waited for. SDK frame journal drains and harness bookkeeping sit outside CPU work timers. `draw` is the SDK Window::draw interval within that callback. No GPU renderer is attached to the CPU context. Input-to-settled latency includes the measured frame sequence and harness bookkeeping; it is unpaced headless wall time, without vsync or network latency. The CPU gate checks each frame ordinal and the worst frame per input in every run."
		: "`draw` times a forced full refresh: render, layout, prepaint, scene paint, and element arena cleanup. It excludes setup, warm-up, target lookup, scene hashing and tasks scheduled by that draw. A view's `frame` column is this CPU redraw cost. For interactions that column is `step`: one input event and all tasks and redraws it causes until the test executor parks. This is input-to-settled CPU latency, potentially spanning several draws, not one frame. The separate forced `draw` is not a measured share of `step`. Fixture reads answer immediately; no cluster or network latency is included.",
	"The first warm-up redraw is reported separately as `first warm-up draw`; setup has already drawn the view, so it is not a cold open. Repeated view draws force actual work rather than timing clean idle windows. Interaction `changed` counts compare painted quads and the input probe outside the timer; inspect this count before trusting an interaction. `gpu`, when available, re-encodes the retained scene after the measured production submit and waits for RGBA readback. It can include previously queued work, so it is not isolated GPU execution or an additive display latency. Without a headless renderer `draw` excludes GPU submit, execution, composition and present, so CPU results cannot prove displayed 120 fps. This inventory covers only cases.rs on this branch, not every possible product interaction or concurrently ported pages.",
	!gpu
		? "GPU measurement disabled for this CPU-only run."
		: result.gpuFloor
			? `GPU floor (empty scene, same readback, pooled across runs): p50 ${ms(result.gpuFloor.p50)} ms, p95 ${ms(result.gpuFloor.p95)} ms. Adapter identity and hardware versus software rendering are not exposed by this test API. Readback drains submissions between inputs, so this replay does not reproduce display pacing or sustained queue pressure. Display present is unmeasured.`
			: "Offscreen rendering unavailable or failed; inspect the raw run log. No GPU or present performance claim is made.",
	...(gpu
		? [
				`Offscreen physical pixel sizes: ${gpuSizes.length ? gpuSizes.join(", ") : "not recorded by this binary; inspect the PNG artifacts"}.`,
			]
		: []),
	"",
	"## Fixture sizes",
	"",
	...Object.entries(result.sizes).map(
		([label, size]) => `- ${label}: ${size.count} (${size.source})`,
	),
	"",
	"## Frames",
	"",
	"| Case | Kind | CPU p50 | p95 | p99 | max | p95 spread | draw p95 | step p95 | renderer CPU p95 | gpu p50 | gpu p95 | changed | halves p50 | first warm-up draw | CPU budget in every run |",
	"| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
	...summaries.map((summary) => {
		const frame = summary.metrics.frame;
		const over = overBudget(summary);
		return `| ${summary.id} | ${summary.kind} | ${ms(frame?.median.p50)} | ${ms(frame?.median.p95)} | ${ms(frame?.median.p99)} | ${ms(frame?.maximum ?? frame?.median.max)} | ${ms(frame?.p95Spread[0])}-${ms(frame?.p95Spread[1])} | ${ms(summary.metrics.draw?.median.p95)} | ${ms(summary.metrics.step?.median.p95)} | ${ms(summary.metrics.gpuFrame?.median.p95)} | ${ms(summary.metrics.gpu?.median.p50)} | ${ms(summary.metrics.gpu?.median.p95)} | ${summary.changed}/${summary.frames} | ${ms(frame?.halves[0])} / ${ms(frame?.halves[1])} | ${ms(summary.coldDrawMs)} | ${over ? "over" : "ok"} |`;
	}),
];
if (result.measurementVersion === 2) {
	lines.push(
		"",
		"## Interaction settling",
		"",
		"| Case | settled p50 ms | p95 | p99 | max | p95 spread | draws p50 | p95 | max | completion presents p50/max | input thread CPU p95 ms | runqueue p95 ms | keepalive CPU p95 ms | CPU p95 by frame ordinal, with spread |",
		"| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
	);
	for (const summary of summaries.filter(
		(entry) => entry.kind === "interaction",
	)) {
		const settled = summary.metrics.step;
		const draws = summary.framesToSettle;
		lines.push(
			`| ${summary.id} | ${ms(settled?.median.p50)} | ${ms(settled?.median.p95)} | ${ms(settled?.median.p99)} | ${ms(settled?.maximum)} | ${ms(settled?.p95Spread[0])}-${ms(settled?.p95Spread[1])} | ${draws?.median.p50} | ${draws?.median.p95} | ${draws?.maximum} | ${summary.completionPresentations?.median.p50}/${summary.completionPresentations?.maximum} | ${ms(summary.metrics.threadCpu?.median.p95)} | ${ms(summary.metrics.runqueue?.median.p95)} | ${ms(summary.metrics.presentation?.median.p95)} | ${summary.frameOrdinals.map((metric, index) => `${index + 1}: ${ms(metric.median.p95)} (${ms(metric.p95Spread[0])}-${ms(metric.p95Spread[1])})`).join("; ")} |`,
		);
	}
}

if (summaries.some((entry) => entry.metrics.gpuFrame)) {
	lines.push(
		"",
		"## Renderer-enabled CPU",
		"",
		"These frames include the headless renderer's CPU encode and submit work. Present to a physical display is unavailable.",
		"",
		"| Case | p50 ms | p95 | p99 | max | p95 spread | worst input p95 spread | encode/submit p95 | input thread CPU p95 | runqueue p95 | keepalive CPU p95 | CPU p95 by frame ordinal, with spread |",
		"| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
	);
	for (const summary of summaries) {
		const frame = summary.metrics.gpuFrame;
		if (!frame) continue;
		const worst = summary.metrics.gpuWorst;
		lines.push(
			`| ${summary.id} | ${ms(frame.median.p50)} | ${ms(frame.median.p95)} | ${ms(frame.median.p99)} | ${ms(frame.maximum)} | ${ms(frame.p95Spread[0])}-${ms(frame.p95Spread[1])} | ${ms(worst?.p95Spread[0])}-${ms(worst?.p95Spread[1])} | ${ms(summary.metrics.gpuSubmit?.median.p95)} | ${ms(summary.metrics.gpuThreadCpu?.median.p95)} | ${ms(summary.metrics.gpuRunqueue?.median.p95)} | ${ms(summary.metrics.gpuPresentation?.median.p95)} | ${summary.gpuFrameOrdinals.map((metric, index) => `${index + 1}: ${ms(metric.median.p95)} (${ms(metric.p95Spread[0])}-${ms(metric.p95Spread[1])})`).join("; ")} |`,
		);
	}
}

if (summaries.some((entry) => entry.metrics.gpuStep)) {
    lines.push("", "## Renderer-enabled interaction settling", "",
        "| Case | settled p50 ms | p95 | p99 | max | p95 spread | draws p50 | p95 | max | completion presents p50/max |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for (const entry of summaries.filter(entry => entry.kind === "interaction")) {
        const time = entry.metrics.gpuStep;
        const draws = entry.gpuFramesToSettle;
        if (!time) continue;
        lines.push(`| ${entry.id} | ${ms(time.median.p50)} | ${ms(time.median.p95)} | ${ms(time.median.p99)} | ${ms(time.maximum)} | ${ms(time.p95Spread[0])}-${ms(time.p95Spread[1])} | ${draws?.median.p50} | ${draws?.median.p95} | ${draws?.maximum} | ${entry.gpuCompletionPresentations?.median.p50}/${entry.gpuCompletionPresentations?.maximum} |`);
    }
}

if (compare) {
	const previous: typeof result = JSON.parse(await readFile(compare, "utf8"));
	if ((previous.measurementVersion ?? 1) !== result.measurementVersion)
		throw new Error(
			"Cannot compare different measurement boundaries; rerun the saved source with this harness",
		);
	lines.push(
		"",
		`## Before and after`,
		"",
		`Before: ${compare}`,
		"",
		"| Case | before p50 | p95 | p99 | max | p95 spread | after p50 | p95 | p99 | max | p95 spread |",
		"| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
	);
	for (const summary of summaries) {
		const previousMetric = previous.summaries.find(
			(entry) => entry.id === summary.id,
		)?.metrics.frame;
		const before = previousMetric?.median;
		const after = summary.metrics.frame?.median;
		const spread = (metric: typeof previousMetric) =>
			`${ms(metric?.p95Spread[0])}-${ms(metric?.p95Spread[1])}`;
		lines.push(
			`| ${summary.id} | ${ms(before?.p50)} | ${ms(before?.p95)} | ${ms(before?.p99)} | ${ms(previousMetric?.maximum ?? before?.max)} | ${spread(previousMetric)} | ${ms(after?.p50)} | ${ms(after?.p95)} | ${ms(after?.p99)} | ${ms(summary.metrics.frame?.maximum ?? after?.max)} | ${spread(summary.metrics.frame)} |`,
		);
	}
}
await writeFile(join(out, "report.md"), `${lines.join("\n")}\n`);
console.log(lines.slice(lines.indexOf("## Frames")).join("\n"));
console.log(`Frame artifacts: ${out}`);
const failures = summaries.filter(overBudget);
if (failures.length) {
	console.error(
		`CPU frame budget exceeded: ${failures.map((entry) => entry.id).join(", ")}`,
	);
	process.exitCode = 1;
}
