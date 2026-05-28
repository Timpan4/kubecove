const DEV_ORIGIN = "http://localhost:1420";
const VITE_CLIENT_URL = `${DEV_ORIGIN}/@vite/client`;
const VITE_CLIENT_MARKERS = ["import.meta.hot", "createHotContext", "/@vite/client"];

async function fetchText(url: string): Promise<string | null> {
	const controller = new AbortController();
	const timeout = setTimeout(() => controller.abort(), 750);
	try {
		const response = await fetch(url, { signal: controller.signal });
		if (!response.ok) return null;
		return await response.text();
	} catch {
		return null;
	} finally {
		clearTimeout(timeout);
	}
}

async function hasKubeCoveViteServer(): Promise<boolean> {
	const [rootHtml, viteClient] = await Promise.all([
		fetchText(DEV_ORIGIN),
		fetchText(VITE_CLIENT_URL),
	]);
	return rootHtml?.includes("<title>KubeCove</title>") === true &&
		VITE_CLIENT_MARKERS.some((marker) => viteClient?.includes(marker));
}

async function hasAnyServer(): Promise<boolean> {
	return (await fetchText(DEV_ORIGIN)) !== null;
}

async function stayAlive(): Promise<never> {
	await new Promise(() => undefined);
	throw new Error("unreachable");
}

if (await hasKubeCoveViteServer()) {
	console.log(`[kubecove:dev] Reusing existing Vite server at ${DEV_ORIGIN}.`);
	await stayAlive();
}

if (await hasAnyServer()) {
	console.error(
		`[kubecove:dev] Port 1420 is already in use, but ${DEV_ORIGIN} does not look like the KubeCove Vite dev server.`,
	);
	console.error("[kubecove:dev] Stop that process, then run bun run tauri dev again.");
	process.exit(1);
}

const child = Bun.spawn(["bun", "run", "dev"], {
	stdin: "inherit",
	stdout: "inherit",
	stderr: "inherit",
});

for (const signal of ["SIGINT", "SIGTERM"] as const) {
	process.on(signal, () => {
		child.kill(signal);
	});
}

process.exit(await child.exited);
