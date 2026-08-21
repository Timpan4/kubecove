const rawArgs = Bun.argv.slice(2);
const args: string[] = [];
for (let index = 0; index < rawArgs.length; index += 1) {
	const arg = rawArgs[index];
	if (["--host", "--hostname", "--port"].includes(arg) && rawArgs[index + 1]) {
		args.push(`${arg}=${rawArgs[index + 1]}`);
		index += 1;
	} else {
		args.push(arg);
	}
}
const hasOption = (name: string): boolean =>
	args.some((arg) => arg === name || arg.startsWith(`${name}=`));

if (!hasOption("--port")) args.push("--port=1430");
const tauriHost = process.env.TAURI_DEV_HOST?.trim();
if (tauriHost && !hasOption("--host") && !hasOption("--hostname")) {
	args.push(`--host=${tauriHost}`);
}

const child = Bun.spawn(["bun", "index.html", "--console", ...args], {
	env: {
		...process.env,
		NODE_ENV: "development",
		KUBECOVE_PUBLIC_DEV: "true",
		KUBECOVE_PUBLIC_RELEASE_CHANNEL: "dev",
	},
	stdin: "inherit",
	stdout: "inherit",
	stderr: "inherit",
});

for (const signal of ["SIGINT", "SIGTERM"] as const) {
	process.on(signal, () => child.kill(signal));
}

process.exit(await child.exited);
