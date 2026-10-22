// Summarizes a dev interaction recording written by scripts/dev-interactions-plugin.ts.
// Usage: bun run perf:interactions [file.jsonl]  (defaults to the newest dev recording;
// a report copied from Settings -> Diagnostics uses the same JSON-lines format)
import { readdir, readFile } from "node:fs/promises";
import { resolve } from "node:path";
import type { RecordedInteraction } from "../src/lib/interaction-recorder.ts";
import { DEV_INTERACTIONS_DIR } from "./dev-interactions-plugin.ts";

async function newestRecording(): Promise<string> {
	const files = (await readdir(DEV_INTERACTIONS_DIR)).filter((name) => name.endsWith(".jsonl")).sort();
	const newest = files.at(-1);
	if (!newest) throw new Error(`No recordings in ${DEV_INTERACTIONS_DIR}`);
	return resolve(DEV_INTERACTIONS_DIR, newest);
}

const file = process.argv[2] ? resolve(process.argv[2]) : await newestRecording();
const interactions: RecordedInteraction[] = (await readFile(file, "utf8"))
	.split("\n")
	.filter((line) => line.trim() !== "")
	.map((line) => JSON.parse(line));

console.log(`${file}\n${interactions.length} interactions\n`);
console.log("id  time      kind    to-paint ms         dropped  max-frame  dom +/-/attr/text  top source  | commands");
const totals = new Map<string, number>();
for (const interaction of interactions) {
	const { dom, frames } = interaction;
	for (const { source, mutations } of dom.sources) totals.set(source, (totals.get(source) ?? 0) + mutations);
	const toPaint = interaction.inputToPaint.map((entry) => `${entry.type}=${entry.ms}`).join(" ") || "-";
	const top = dom.sources[0] ? `${dom.sources[0].source} (${dom.sources[0].mutations})` : "-";
	const commands = (interaction.commands ?? []).map((command) => `${command.cmd} ${command.ms}ms${command.ok ? "" : " ERR"}`).join(", ");
	console.log(
		[
			String(interaction.id).padEnd(3),
			new Date(interaction.wallTime).toLocaleTimeString("en-GB"),
			interaction.kind.padEnd(7),
			toPaint.padEnd(19),
			String(frames.droppedFrames).padStart(7),
			`${frames.maxMs}`.padStart(10),
			`${dom.added}/${dom.removed}/${dom.attributes}/${dom.text}`.padStart(18),
			top,
			commands ? `| ${commands}` : "",
		].join("  "),
	);
	console.log(`    target: ${interaction.target}`);
}

console.log("\nDOM mutations by Svelte source, whole session:");
for (const [source, mutations] of [...totals.entries()].sort((a, b) => b[1] - a[1])) {
	console.log(`${String(mutations).padStart(8)}  ${source}`);
}
