#!/usr/bin/env bun
// Checks the resource-watch lifecycle: TLC on the model of the current code, TLC on the
// pre-#635 variant (which must still fail, so the rules can catch a bug), then a replay of
// the current model's whole state graph against the real observeResourceScope.
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { replay } from "./replay";

const modelDir = resolve(import.meta.dir, "../model");
// The pins the statechart skill installs with mise: TLC 1.7.4 by checksum and Temurin 25.
const tlcTool =
	"http:tla2tools[url=https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar,checksum=sha256:936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88]@1.7.4";
const javaTool = "java@temurin-25";

function findTlc() {
	if (process.env.TLA2TOOLS) return { jar: process.env.TLA2TOOLS, java: process.env.JAVA ?? "java" };
	const where = spawnSync("mise", ["where", tlcTool], { encoding: "utf8" });
	const java = spawnSync("mise", ["which", "java", "--tool", javaTool], { encoding: "utf8" });
	if (where.status !== 0 || java.status !== 0)
		throw new Error(`TLC is not installed. Install it with: mise install ${javaTool} '${tlcTool}'\nOr set TLA2TOOLS to tla2tools.jar and JAVA to a Java 11+ binary.`);
	return { jar: join(where.stdout.trim(), "tla2tools.jar"), java: process.env.JAVA ?? java.stdout.trim() };
}

const work = mkdtempSync(join(tmpdir(), "verify-resource-watch-"));
const { jar, java } = findTlc();

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

const results: Array<{ check: string; ok: boolean; detail: string }> = [];

const current = tlc("ResourceWatch.cfg", ["-dump", "dot,actionlabels", join(work, "states")]);
const states = /^\d+ states generated, (\d+) distinct states found/m.exec(current.output)?.[1];
results.push({
	check: "TLC on the current code's model: NoSilentDeadStream, LostStreamRecovers, StartBoundRoom, TypeOK, no deadlock",
	ok: current.status === 0 && current.output.includes("No error has been found"),
	detail: `${states ?? "?"} distinct states; TLC output ${current.file}`,
});

const safety = tlc("Pre635Safety.cfg");
results.push({
	check: "TLC on the pre-#635 model still breaks NoSilentDeadStream",
	ok: safety.output.includes("Invariant NoSilentDeadStream is violated"),
	detail: `TLC output ${safety.file}`,
});

const liveness = tlc("Pre635Liveness.cfg");
results.push({
	check: "TLC on the pre-#635 model still breaks LostStreamRecovers",
	ok: liveness.output.includes("Temporal properties were violated"),
	detail: `TLC output ${liveness.file}`,
});

if (results[0].ok) {
	const conformance = await replay(join(work, "states.dot"));
	results.push({
		check: "Replay of every model step against src/lib/resource-watch.ts",
		ok: conformance.ok,
		detail: conformance.ok
			? `${conformance.replayed} edge replays over ${conformance.states} states and ${conformance.edges} edges`
			: `the code and the model differ after: ${conformance.steps.join(" -> ")}\n    ${conformance.differences.join("\n    ")}`,
	});
} else results.push({ check: "Replay of every model step against src/lib/resource-watch.ts", ok: false, detail: "skipped: TLC did not pass on the current model" });

for (const result of results) console.log(`${result.ok ? "PASS" : "FAIL"} ${result.check}\n    ${result.detail}`);
console.log(`Work folder: ${work}`);
process.exit(results.every((result) => result.ok) ? 0 : 1);
