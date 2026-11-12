import { readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = new URL("../", import.meta.url);

// Seconds in the "Run benchmarks" step for each file from GitHub Actions run
// 43308253138, job timing JSON captured 2026-11-12. Step duration isolates
// benchmark execution from checkout, dependency installation, and runner queue.
// Refresh these weights from comparable hosted runs when benchmark files change.
const benchmarkDurationSeconds = {
	"benchmarks/argo-operation.bench.ts": 24,
	"benchmarks/argo-resource-display.bench.ts": 125,
	"benchmarks/argo-workspace.bench.ts": 30,
	"benchmarks/command-palette.bench.ts": 33,
	"benchmarks/detail-metadata.bench.ts": 62,
	"benchmarks/diagnostics.bench.ts": 24,
	"benchmarks/incidents.bench.ts": 37,
	"benchmarks/live-sessions.bench.ts": 36,
	"benchmarks/major-surfaces.bench.ts": 44,
	"benchmarks/rbac-surface.bench.ts": 101,
	"benchmarks/resource-detail.bench.ts": 45,
	"benchmarks/resource-metrics.bench.ts": 28,
	"benchmarks/resource-planning.bench.ts": 29,
	"benchmarks/resource-table.bench.ts": 250,
	"benchmarks/resource-workspace-state.bench.ts": 40,
	"benchmarks/sidebar-tree.bench.ts": 37,
	"benchmarks/topology-large.bench.ts": 41,
	"benchmarks/topology-layout.bench.ts": 46,
	"benchmarks/topology-map.bench.ts": 60,
	"benchmarks/topology.bench.ts": 343,
	"benchmarks/workspace-health-entries.bench.ts": 32,
	"benchmarks/yaml-diff.bench.ts": 32,
	"benchmarks/yaml-editor.bench.ts": 46,
	"benchmarks/yaml-format.bench.ts": 52,
	"benchmarks/yaml-secrets.bench.ts": 83,
};

export function benchmarkFiles(directory = "benchmarks") {
	return readdirSync(new URL(`${directory}/`, root), { withFileTypes: true })
		.flatMap((entry) => {
			const path = `${directory}/${entry.name}`;
			return entry.isDirectory()
				? benchmarkFiles(path)
				: entry.isFile() && entry.name.endsWith(".bench.ts")
					? [path]
					: [];
		})
		.sort();
}

export function benchmarkGroups(files, count) {
	const durations = files.map((file) => {
		const seconds = benchmarkDurationSeconds[file];
		if (seconds === undefined)
			throw new Error(`Missing measured duration for benchmark file: ${file}`);
		return { file, seconds };
	});
	const groups = Array.from({ length: count }, () => ({ seconds: 0, files: [] }));
	for (const item of durations.sort(
		(left, right) => right.seconds - left.seconds || left.file.localeCompare(right.file),
	)) {
		let lightest = 0;
		for (let index = 1; index < groups.length; index++) {
			if (groups[index].seconds < groups[lightest].seconds) lightest = index;
		}
		groups[lightest].seconds += item.seconds;
		groups[lightest].files.push(item.file);
	}
	return groups;
}

export function benchmarkSelection(env = process.env) {
	const files = benchmarkFiles();
	const file = env.KUBECOVE_BENCHMARK_FILE;
	const suite = env.KUBECOVE_BENCHMARK_SUITE;
	const group = env.KUBECOVE_BENCHMARK_GROUP;
	if ([file, suite, group].filter(Boolean).length > 1)
		throw new Error("Select only one benchmark file, suite or group");
	if (group) {
		// Longest measured files go to the currently lightest group.
		const match = /^(\d+)\/(\d+)$/.exec(group);
		const [index, count] = [Number(match?.[1]), Number(match?.[2])];
		if (!match || count < 1 || index >= count)
			throw new Error(`KUBECOVE_BENCHMARK_GROUP must be <index>/<count>: ${group}`);
		return {
			include: benchmarkGroups(files, count)[index].files.sort(),
			exclude: [],
		};
	}
	if (file) {
		if (!files.includes(file))
			throw new Error(`Unknown benchmark file: ${file}`);
		return { include: [file], exclude: [] };
	}
	if (suite === "topology") {
		return {
			include: files.filter((path) => /\/topology[^/]*\.bench\.ts$/.test(path)),
			exclude: [],
		};
	}
	if (suite === "other") {
		return {
			include: files.filter(
				(path) => !/\/topology[^/]*\.bench\.ts$/.test(path),
			),
			exclude: [],
		};
	}
	if (suite) throw new Error(`Unknown benchmark suite: ${suite}`);
	return { include: files, exclude: [] };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	process.stdout.write(`${JSON.stringify(benchmarkFiles())}\n`);
}
