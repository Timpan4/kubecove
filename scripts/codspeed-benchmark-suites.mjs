import { readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = new URL("../", import.meta.url);

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

export function benchmarkSelection(env = process.env) {
	const files = benchmarkFiles();
	const file = env.KUBECOVE_BENCHMARK_FILE;
	const suite = env.KUBECOVE_BENCHMARK_SUITE;
	if (file && suite)
		throw new Error("Select a benchmark file or suite, not both");
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
