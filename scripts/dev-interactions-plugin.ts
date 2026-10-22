import { appendFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import type { Plugin } from "vite";

export const DEV_INTERACTIONS_DIR = resolve(import.meta.dirname, "../e2e/artifacts/dev-interactions");

// Parses a posted body into the JSON line to append, or null when it lacks the fields
// bun run perf:interactions reads, so a stray post cannot break the report.
function parseInteractionLine(body: string): string | null {
	const entry = Object(JSON.parse(body));
	const frames = Object(entry.frames);
	const dom = Object(entry.dom);
	const valid = Number.isFinite(entry.id) && [entry.kind, entry.wallTime, entry.target].every((field) => String(field) === field) &&
		Array.isArray(entry.inputToPaint) &&
		Array.from(entry.inputToPaint, Object).every((paint) => String(paint.type) === paint.type && Number.isFinite(paint.ms)) &&
		Number.isFinite(frames.droppedFrames) && Number.isFinite(frames.maxMs) && Array.isArray(dom.sources) &&
		Array.from(dom.sources, Object).every((source) => String(source.source) === source.source && Number.isFinite(source.mutations));
	return valid ? JSON.stringify(entry) : null;
}

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
				// A JSON content type forces a CORS preflight, which Vite's localhost-only
				// default denies to foreign origins; simple text/plain posts are rejected.
				if (!request.headers["content-type"]?.startsWith("application/json")) {
					response.statusCode = 415;
					response.end();
					return;
				}
				const chunks: Buffer[] = [];
				request.on("data", (chunk: Buffer) => chunks.push(chunk));
				request.on("end", async () => {
					try {
						const line = parseInteractionLine(Buffer.concat(chunks).toString("utf8"));
						if (!line) throw new Error("not an interaction entry");
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
