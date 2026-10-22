import { appendFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import type { Plugin } from "vite";

export const DEV_INTERACTIONS_DIR = resolve(import.meta.dirname, "../e2e/artifacts/dev-interactions");

// Dev-server sink for src/lib/interaction-recorder.ts: appends each posted
// interaction as one JSON line to a per-server-start file.
export function devInteractionsPlugin(endpoint: string): Plugin {
	const file = resolve(DEV_INTERACTIONS_DIR, `${new Date().toISOString().replaceAll(":", "-")}.jsonl`);
	return {
		name: "kubecove-dev-interactions",
		apply: "serve",
		configureServer(server) {
			server.httpServer?.once("listening", () => {
				server.config.logger.info(`[kubecove:dev-interactions] writing ${file}`);
			});
			server.middlewares.use(endpoint, (request, response) => {
				if (request.method !== "POST") {
					response.statusCode = 405;
					response.end();
					return;
				}
				const chunks: Buffer[] = [];
				request.on("data", (chunk: Buffer) => chunks.push(chunk));
				request.on("end", async () => {
					try {
						const line = JSON.stringify(JSON.parse(Buffer.concat(chunks).toString("utf8")));
						await mkdir(DEV_INTERACTIONS_DIR, { recursive: true });
						await appendFile(file, `${line}\n`);
						response.statusCode = 204;
					} catch (error) {
						server.config.logger.warn(`[kubecove:dev-interactions] rejected entry: ${String(error)}`);
						response.statusCode = 400;
					}
					response.end();
				});
			});
		},
	};
}
