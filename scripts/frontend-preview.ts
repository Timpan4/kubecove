import { resolve, sep } from "node:path";

const root = resolve("dist");
const port = Number.parseInt(process.env.PORT ?? "4173", 10);
const index = Bun.file(resolve(root, "index.html"));

const server = Bun.serve({
	hostname: "127.0.0.1",
	port,
	async fetch(request) {
		const pathname = decodeURIComponent(new URL(request.url).pathname);
		const candidate = resolve(root, `.${pathname === "/" ? "/index.html" : pathname}`);
		if (candidate !== root && !candidate.startsWith(`${root}${sep}`)) {
			return new Response("Forbidden", { status: 403 });
		}
		const file = Bun.file(candidate);
		return new Response((await file.exists()) ? file : index);
	},
});

console.log(`Bun preview server listening on ${server.url}`);
