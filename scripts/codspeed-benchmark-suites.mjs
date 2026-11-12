import { readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = new URL("../", import.meta.url);

// File completion intervals from run 43322953935 include imports, fixtures and
// test execution. Each interval starts at the previous file completion, or the
// Vitest start timestamp for the first file. Split chunks repeat file preparation.
const benchmarkDurationSeconds = {
	"benchmarks/command-palette.bench.ts": 13.964,
	"benchmarks/resource-planning.bench.ts": 13.143,
	"benchmarks/resource-detail.bench.ts": 17.077,
	"benchmarks/resource-workspace-state.bench.ts": 11.983,
	"benchmarks/live-sessions.bench.ts": 9.21,
	"benchmarks/diagnostics.bench.ts": 7.167,
	"benchmarks/argo-workspace.bench.ts": 9.227,
	"benchmarks/rbac-surface.bench.ts": 62.393,
	"benchmarks/topology-map.bench.ts": 28.291,
	"benchmarks/sidebar-tree.bench.ts": 11.966,
	"benchmarks/yaml-diff.bench.ts": 8.819,
	"benchmarks/yaml-editor.bench.ts": 20.106,
	"benchmarks/argo-operation.bench.ts": 7.334,
	"benchmarks/yaml-secrets.bench.ts": 36.308,
	"benchmarks/topology-large.bench.ts": 22.088,
	"benchmarks/major-surfaces.bench.ts": 33.723,
	"benchmarks/incidents.bench.ts": 31.577,
	"benchmarks/workspace-health-entries.bench.ts": 23.914,
	"benchmarks/topology-layout.bench.ts": 48.261,
	"benchmarks/resource-metrics.bench.ts": 13.263,
	"benchmarks/argo-resource-display.bench.ts": 130.848,
	"benchmarks/detail-metadata.bench.ts": 8.971,
	"benchmarks/yaml-format.bench.ts": 31.278,
};

// Run 43322953935 group 0: 287 s step minus 271.59 s Vitest duration.
// This observed warm-action overhead is a projection, not an enforced timeout.
const actionOverheadSeconds = 15.41;
const splitBenchmarks = [
	{
		"file": "benchmarks/topology.bench.ts",
		"names": [
			"F07 graph layout and F09 retained topology selection (500 apps / 1k nodes) > F07 layout construction per selection (50 full rebuilds)"
		],
		"seconds": 114.337
	},
	{
		"file": "benchmarks/topology.bench.ts",
		"names": [
			"F07 graph layout and F09 retained topology selection (500 apps / 1k nodes) > F09 selection + resize (rebuild view, 50 updates)"
		],
		"seconds": 123.883
	},
	{
		"file": "benchmarks/topology.bench.ts",
		"names": [
			"F07 graph layout and F09 retained topology selection (500 apps / 1k nodes) > F07 ownership layout (build once)",
			"F07 graph layout and F09 retained topology selection (500 apps / 1k nodes) > F09 indexed selection (50 updates)",
			"F07 graph layout and F09 retained topology selection (500 apps / 1k nodes) > F09 selection + resize (retained layout, 50 updates)",
			"F07 graph layout and F09 retained topology selection (500 apps / 1k nodes) > F10 full and focused fit plans across viewport geometry"
		],
		"seconds": 17.648
	},
	{
		"file": "benchmarks/resource-table.bench.ts",
		"names": [
			"F01 F02 F03 resource arrival and retained table state (10k rows) > buildResourceSearchIndex",
			"F01 F02 F03 resource arrival and retained table state (10k rows) > filterResourceSearchIndex (search + gitops owner)",
			"F01 F02 F03 resource arrival and retained table state (10k rows) > buildResourceTableModel (search + health + sort + groups)",
			"F01 F02 F03 resource arrival and retained table state (10k rows) > buildResourceTableModel (gitops owner filtered)",
			"F01 F02 F03 resource arrival and retained table state (10k rows) > F01 multi-hop ownership projection on arrival",
			"F01 F02 F03 resource arrival and retained table state (10k rows) > F03 kind filter + F01 projection for versioned CRDs",
			"F01 F02 F03 resource arrival and retained table state (10k rows) > buildResourceTableModel repeated variants (reused projection)",
			"F01 F02 F03 resource arrival and retained table state (10k rows) > F02 row selection (reused display and page)",
			"F01 F02 F03 resource arrival and retained table state (10k rows) > F02 retained model on multi-hop ownership projection",
			"multi-context resource table (10k rows x 1 contexts) > buildResourceSearchIndex, cached rows (1 contexts)",
			"multi-context resource table (10k rows x 1 contexts) > copy rows, control for the fresh-rows index (1 contexts)",
			"multi-context resource table (10k rows x 1 contexts) > buildResourceSearchIndex, fresh rows incl. copy (1 contexts)",
			"multi-context resource table (10k rows x 1 contexts) > buildResourceTableDisplay (search + health + sort + groups, 1 contexts)",
			"multi-context resource table (10k rows x 1 contexts) > buildResourceTableDisplay (all rows by name, 1 contexts)",
			"multi-context resource table (10k rows x 2 contexts) > buildResourceSearchIndex, cached rows (2 contexts)",
			"multi-context resource table (10k rows x 2 contexts) > copy rows, control for the fresh-rows index (2 contexts)",
			"multi-context resource table (10k rows x 2 contexts) > buildResourceSearchIndex, fresh rows incl. copy (2 contexts)",
			"multi-context resource table (10k rows x 2 contexts) > buildResourceTableDisplay (search + health + sort + groups, 2 contexts)",
			"multi-context resource table (10k rows x 2 contexts) > buildResourceTableDisplay (all rows by name, 2 contexts)"
		],
		"seconds": 111.734
	},
	{
		"file": "benchmarks/resource-table.bench.ts",
		"names": [
			"multi-context resource table (10k rows x 4 contexts) > buildResourceSearchIndex, cached rows (4 contexts)",
			"multi-context resource table (10k rows x 4 contexts) > copy rows, control for the fresh-rows index (4 contexts)",
			"multi-context resource table (10k rows x 4 contexts) > buildResourceSearchIndex, fresh rows incl. copy (4 contexts)",
			"multi-context resource table (10k rows x 4 contexts) > buildResourceTableDisplay (search + health + sort + groups, 4 contexts)",
			"multi-context resource table (10k rows x 4 contexts) > buildResourceTableDisplay (all rows by name, 4 contexts)"
		],
		"seconds": 69.101
	}
];

function escapePattern(name) {
	return name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

const splitSuites = [...new Set(splitBenchmarks.flatMap((item) =>
	item.names.map((name) => name.slice(0, name.lastIndexOf(" > "))),
))];

function namePattern(items) {
	const names = items.flatMap((item) => item.names ?? []).map(escapePattern);
	const otherSuites = `(?!(?:${splitSuites.map(escapePattern).join("|")}) > ).*`;
	return `^(?:${[otherSuites, ...names].join("|")})$`;
}

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
	const durations = files.flatMap((file) => {
		const chunks = splitBenchmarks.filter((item) => item.file === file);
		if (chunks.length) return chunks;
		const seconds = benchmarkDurationSeconds[file];
		if (seconds === undefined)
			throw new Error(`Missing measured duration for benchmark file: ${file}`);
		return [{ file, seconds, names: [] }];
	});
	const groups = Array.from({ length: count }, () => ({ seconds: actionOverheadSeconds, items: [] }));
	for (const item of durations.sort(
		(left, right) => right.seconds - left.seconds || left.file.localeCompare(right.file),
	)) {
		let lightest = 0;
		for (let index = 1; index < groups.length; index++) {
			if (groups[index].seconds < groups[lightest].seconds) lightest = index;
		}
		groups[lightest].seconds += item.seconds;
		groups[lightest].items.push(item);
	}
	return groups.map(({ seconds, items }) => ({
		seconds,
		files: [...new Set(items.map((item) => item.file))].sort(),
		testNamePattern: namePattern(items),
	}));
}

export function benchmarkSelection(env = process.env) {
	const files = benchmarkFiles();
	const file = env.KUBECOVE_BENCHMARK_FILE;
	const suite = env.KUBECOVE_BENCHMARK_SUITE;
	const group = env.KUBECOVE_BENCHMARK_GROUP;
	if ([file, suite, group].filter(Boolean).length > 1)
		throw new Error("Select only one benchmark file, suite or group");
	if (group) {
		// Longest measured chunks go to the currently lightest group.
		const match = /^(\d+)\/(\d+)$/.exec(group);
		const [index, count] = [Number(match?.[1]), Number(match?.[2])];
		if (!match || count < 1 || index >= count)
			throw new Error(`KUBECOVE_BENCHMARK_GROUP must be <index>/<count>: ${group}`);
		const selected = benchmarkGroups(files, count)[index];
		return {
			include: selected.files,
			testNamePattern: selected.testNamePattern,
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
