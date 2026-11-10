// Generates the static font instances the GPUI client bundles. GPUI's text
// system cannot select weights from variable fonts, so each weight the web
// stylesheet uses becomes a static TTF cut from the same pinned fontsource
// files the Svelte frontend loads.
// Requires woff2_decompress and fonttools. Usage: bun scripts/gpui-fonts.ts
import { copyFile, mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const families = [
	{ pkg: "inter", file: "inter-latin-wght-normal", weights: [400, 500, 600, 700] },
	{ pkg: "geist", file: "geist-latin-wght-normal", weights: [500, 600] },
	{ pkg: "geist-mono", file: "geist-mono-latin-wght-normal", weights: [400, 500] },
];
const output = resolve("src-tauri/gpui/assets/fonts");

async function run(command: string[]) {
	const child = Bun.spawn(command, { stdout: "ignore", stderr: "pipe" });
	const [code, stderr] = await Promise.all([child.exited, new Response(child.stderr).text()]);
	if (code !== 0) throw new Error(`${command.join(" ")} exited ${code}: ${stderr}`);
}

await mkdir(output, { recursive: true });
const work = await mkdtemp(join(tmpdir(), "kubecove-fonts-"));
try {
	for (const family of families) {
		const source = resolve("node_modules/@fontsource-variable", family.pkg);
		const woff2 = join(work, `${family.file}.woff2`);
		await copyFile(join(source, "files", `${family.file}.woff2`), woff2);
		await run(["woff2_decompress", woff2]);
		for (const weight of family.weights) {
			await run(["fonttools", "varLib.instancer", join(work, `${family.file}.ttf`), `wght=${weight}`, "--static", "-o", join(output, `${family.pkg}-${weight}.ttf`)]);
		}
		await copyFile(join(source, "LICENSE"), join(output, `${family.pkg}-LICENSE.txt`));
	}
} finally {
	await rm(work, { recursive: true, force: true });
}
console.log(`fonts -> ${output}`);
