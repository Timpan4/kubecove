import { spawnSync } from "node:child_process";
import { performance } from "node:perf_hooks";

type CompilerMode = "on" | "off";

interface BuildResult {
	label: string;
	reactCompiler: CompilerMode;
	status: number;
	durationMs: number;
}

function runBuild(reactCompiler: CompilerMode): BuildResult {
	const started = performance.now();
	const result = spawnSync("bun", ["run", "build"], {
		env: {
			...process.env,
			KUBECOVE_REACT_COMPILER: reactCompiler,
		},
		stdio: "inherit",
	});
	return {
		label: `react-compiler-${reactCompiler}`,
		reactCompiler,
		status: result.status ?? 1,
		durationMs: Number((performance.now() - started).toFixed(2)),
	};
}

const results = [runBuild("on"), runBuild("off")];
const failed = results.find((result) => result.status !== 0);

console.log(JSON.stringify({ results }, null, 2));

if (failed) {
	process.exit(failed.status);
}
