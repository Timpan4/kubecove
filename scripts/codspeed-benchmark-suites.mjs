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
	const group = env.KUBECOVE_BENCHMARK_GROUP;
	if ([file, suite, group].filter(Boolean).length > 1)
		throw new Error("Select only one benchmark file, suite or group");
	if (group) {
		// CI jobs each run every `count`-th file from position `index`.
		const match = /^(\d+)\/(\d+)$/.exec(group);
		const [index, count] = [Number(match?.[1]), Number(match?.[2])];
		if (!match || count < 1 || index >= count)
			throw new Error(`KUBECOVE_BENCHMARK_GROUP must be <index>/<count>: ${group}`);
		return {
			include: files.filter((_, position) => position % count === index),
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
