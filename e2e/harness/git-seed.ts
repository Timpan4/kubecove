import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

export const gitSeedIdentity = {
	name: "KubeCove E2E",
	email: "e2e@kubecove.invalid",
} as const;

export function kindMetricsManifest(source: string) {
	if (source.includes("--kubelet-insecure-tls")) return source;
	const anchor = "        - --kubelet-use-node-status-port";
	if (!source.includes(anchor)) throw new Error("metrics-server manifest shape changed");
	return source.replace(anchor, `${anchor}\n        - --kubelet-insecure-tls`);
}

export async function prepareGitSeed(options: {
	source: string;
	destination: string;
	repositoryUrl: string;
	metricsManifest: string;
	storageManifest: string;
}) {
	await rm(options.destination, { recursive: true, force: true });
	await cp(options.source, options.destination, { recursive: true });
	for await (const relative of new Bun.Glob("**/*.yaml").scan({ cwd: options.destination })) {
		const file = join(options.destination, relative);
		const source = await readFile(file, "utf8");
		await writeFile(file, source.replaceAll("__KUBECOVE_GIT_REPO_URL__", options.repositoryUrl));
	}
	const metrics = join(options.destination, "platform", "metrics", "metrics-server.yaml");
	const storage = join(options.destination, "platform", "storage", "local-path-storage.yaml");
	await mkdir(dirname(metrics), { recursive: true });
	await mkdir(dirname(storage), { recursive: true });
	await writeFile(metrics, kindMetricsManifest(options.metricsManifest));
	await writeFile(storage, options.storageManifest);
}

