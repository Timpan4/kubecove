#!/usr/bin/env bun
// Checks the resource-watch lifecycle: TLC on the model of the current code, TLC on the
// pre-#635 variant (which must still fail, so the rules can catch a bug), then a replay of
// the current model's whole state graph against the real observeResourceScope.
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { replay } from "./replay";

const modelDir = resolve(import.meta.dir, "../model");
// The pins the statechart skill installs with mise: TLC 1.7.4 by checksum and Temurin 25.
const tlcTool =
	"http:tla2tools[url=https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar,checksum=sha256:936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88]@1.7.4";
const javaTool = "java@temurin-25";

// Java runs in the work folder, so relative override paths are resolved from the caller's folder first.
// A bare JAVA command name such as `java` stays a PATH lookup.
const javaOverride = process.env.JAVA && (/[\\/]/.test(process.env.JAVA) ? resolve(process.env.JAVA) : process.env.JAVA);

function findTlc() {
	if (process.env.TLA2TOOLS) return { jar: resolve(process.env.TLA2TOOLS), java: javaOverride || "java" };
	const where = spawnSync("mise", ["where", tlcTool], { encoding: "utf8" });
	const java = spawnSync("mise", ["which", "java", "--tool", javaTool], { encoding: "utf8" });
	if (where.status !== 0 || java.status !== 0)
		throw new Error(`TLC is not installed. Install it with: mise install ${javaTool} '${tlcTool}'\nOr set TLA2TOOLS to tla2tools.jar and JAVA to a Java 11+ binary.`);
	return { jar: join(where.stdout.trim(), "tla2tools.jar"), java: javaOverride || java.stdout.trim() };
}

const { jar, java } = findTlc();
const work = mkdtempSync(join(tmpdir(), "verify-resource-watch-"));

function tlc(config: string, extra: string[] = []) {
	copyFileSync(join(modelDir, "ResourceWatch.tla"), join(work, "ResourceWatch.tla"));
	copyFileSync(join(modelDir, config), join(work, config));
	const run = spawnSync(
		java,
		["-XX:+UseParallelGC", `-Djava.io.tmpdir=${work}`, "-cp", jar, "tlc2.TLC", "-workers", "1", "-metadir", join(work, `meta-${config}`), "-config", config, ...extra, "ResourceWatch.tla"],
		{ cwd: work, encoding: "utf8", env: { ...process.env, LC_ALL: "C.UTF-8" } },
	);
	const output = `${run.stdout}${run.stderr}`;
	writeFileSync(join(work, `${config}.out`), output);
	return { status: run.status, output, file: join(work, `${config}.out`) };
}

// `file` is the TLC output, printed only on failure because a pass deletes the work folder.
const results: Array<{ check: string; ok: boolean; detail: string; file?: string }> = [];

const current = tlc("ResourceWatch.cfg", ["-dump", "dot,actionlabels", join(work, "states")]);
const [, generated, states] = /^(\d+) states generated, (\d+) distinct states found/m.exec(current.output) ?? [];
const initialStates = /^Finished computing initial states: (\d+) distinct states? generated/m.exec(current.output)?.[1];
results.push({
	check: "TLC on the current code's model: NoSilentDeadStream, LostStreamRecovers, StartBoundRoom, TypeOK, no deadlock",
	ok: current.status === 0 && current.output.includes("No error has been found"),
	detail: `${states ?? "?"} distinct states`,
	file: current.file,
});

const safety = tlc("Pre635Safety.cfg");
results.push({
	check: "TLC on the pre-#635 model still breaks NoSilentDeadStream",
	ok: safety.output.includes("Invariant NoSilentDeadStream is violated"),
	detail: "TLC reports the invariant violated",
	file: safety.file,
});

const liveness = tlc("Pre635Liveness.cfg");
results.push({
	check: "TLC on the pre-#635 model still breaks LostStreamRecovers",
	ok: liveness.output.includes("Temporal properties were violated"),
	detail: "TLC reports the temporal property violated",
	file: liveness.file,
});

function replayFailure(conformance: { crashed?: string; graphInvalid?: boolean; steps: string[] }) {
	if (conformance.crashed) return `the replay stopped with an error: ${conformance.crashed}`;
	if (conformance.graphInvalid) return "the state graph cannot be replayed";
	if (conformance.steps.length === 0) return "the code and the model differ in the initial state";
	return `the code and the model differ after: ${conformance.steps.join(" -> ")}`;
}

if (results[0].ok) {
	// Any other error still prints the result lines and keeps the work folder.
	// A count TLC did not print becomes NaN, which matches nothing, so the replay reports the graph as unreadable.
	const expected = { states: Number(states), transitions: Number(generated) - Number(initialStates) };
	const conformance = await replay(join(work, "states.dot"), expected).catch((error: unknown) => ({
		ok: false as const,
		crashed: error instanceof Error ? error.message : String(error),
		replayed: 0,
		steps: [] as string[],
		differences: [] as string[],
		states: 0,
		edges: 0,
	}));
	results.push({
		check: "Replay of every model step against src/lib/resource-watch.ts",
		ok: conformance.ok,
		detail: conformance.ok
			? `${conformance.replayed} edge replays over ${conformance.states} states and ${conformance.edges} edges`
			: [replayFailure(conformance), ...conformance.differences].join("\n    "),
	});
} else results.push({ check: "Replay of every model step against src/lib/resource-watch.ts", ok: false, detail: "skipped: TLC did not pass on the current model" });

for (const result of results) console.log(`${result.ok ? "PASS" : "FAIL"} ${result.check}\n    ${result.detail}${!result.ok && result.file ? `; TLC output ${result.file}` : ""}`);
const passed = results.every((result) => result.ok);
// The work folder holds the TLC traces a failure needs; after a pass nothing in it is needed.
if (passed) rmSync(work, { recursive: true, force: true });
else console.log(`Work folder, kept for the TLC output: ${work}`);
process.exit(passed ? 0 : 1);
