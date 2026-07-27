import { win32 } from "node:path";

const DEFAULT_DEVTOOLS_PORT = "9222";
const CARGO_TARGET_DIR = "CARGO_TARGET_DIR";
const WEBVIEW2_ARGUMENTS = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";

export function tauriEnvironment(
	args: string[],
	baseEnvironment: Record<string, string | undefined> = process.env,
	platform = process.platform,
): Record<string, string | undefined> {
	const environment = { ...baseEnvironment };
	if (platform !== "win32" || args[0] !== "dev") return environment;

	const localAppData = environment.LOCALAPPDATA?.trim();
	if (!environment[CARGO_TARGET_DIR]?.trim() && localAppData) {
		environment[CARGO_TARGET_DIR] = win32.join(
			localAppData,
			"KubeCove",
			"cargo-target",
		);
	}

	const currentArguments = environment[WEBVIEW2_ARGUMENTS]?.trim() ?? "";
	if (/(?:^|\s)--remote-debugging-port(?:=|\s)/.test(currentArguments)) {
		return environment;
	}

	const port = environment.KUBECOVE_DEVTOOLS_PORT?.trim() || DEFAULT_DEVTOOLS_PORT;
	const debugArguments = [
		`--remote-debugging-port=${port}`,
		"--remote-debugging-address=127.0.0.1",
		`--remote-allow-origins=http://127.0.0.1:${port},http://localhost:${port}`,
	];
	environment[WEBVIEW2_ARGUMENTS] = [currentArguments, ...debugArguments]
		.filter(Boolean)
		.join(" ");
	return environment;
}

if (import.meta.main) {
	const args = Bun.argv.slice(2);
	const child = Bun.spawn(["bun", "x", "tauri", ...args], {
		env: tauriEnvironment(args),
		stdin: "inherit",
		stdout: "inherit",
		stderr: "inherit",
	});

	for (const signal of ["SIGINT", "SIGTERM"] as const) {
		process.on(signal, () => child.kill(signal));
	}

	process.exit(await child.exited);
}
