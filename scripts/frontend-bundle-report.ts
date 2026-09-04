import { readFile } from "node:fs/promises";
import { extname, isAbsolute, relative, resolve, sep } from "node:path";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import type { BuildArtifact, BuildMetafile } from "bun";

function portablePath(root: string, file: string): string {
	const path = relative(root, resolve(root, file));
	if (isAbsolute(path) || path === ".." || path.startsWith(`..${sep}`)) {
		throw new Error("Bundle report input is outside the repository");
	}
	return path.split(sep).join("/");
}

function inputPath(file: string): string {
	// Bun can prefix cross-drive Windows inputs with parent segments.
	return resolve(process.platform === "win32" ? file.replace(/^(?:\.\.\/)+(?=[a-z]:\/)/i, "") : file);
}

function owner(path: string): string {
	const dependency = path.lastIndexOf("node_modules/");
	if (dependency >= 0) {
		const parts = path.slice(dependency + "node_modules/".length).split("/");
		return parts[0].startsWith("@") ? parts.slice(0, 2).join("/") : parts[0];
	}
	if (path.startsWith("src/features/")) return path.split("/").slice(0, 3).join("/");
	if (path.startsWith("src/")) return path.split("/").slice(0, 2).join("/");
	return "app-assets";
}

export async function frontendBundleReport(
	root: string,
	outdir: string,
	outputs: BuildArtifact[],
	metafile: BuildMetafile,
) {
	const metadata = new Map(Object.entries(metafile.outputs).map(([path, value]) => [portablePath(root, resolve(outdir, path)), value]));
	const files = await Promise.all(outputs.map(async (output) => {
		const path = portablePath(root, output.path);
		const bytes = new Uint8Array(await output.arrayBuffer());
		let meta = metadata.get(path);
		// Bun omits copied HTML assets from outputs metadata. Match exact source bytes.
		if (!meta && output.kind === "asset") {
			const inputs: BuildMetafile["outputs"][string]["inputs"] = {};
			for (const input of Object.keys(metafile.inputs)) {
				if (extname(input) !== extname(path)) continue;
				if (Buffer.from(bytes).equals(await readFile(inputPath(input)))) inputs[input] = { bytesInOutput: bytes.length };
			}
			if (Object.keys(inputs).length) meta = { bytes: bytes.length, inputs, imports: [], exports: [] };
		}
		if (!meta) throw new Error(`Missing bundle metadata for ${path}`);
		const inputs = Object.entries(meta.inputs).map(([input, value]) => {
			const source = portablePath(root, inputPath(input));
			return { path: source, owner: owner(source), bytesInOutput: value.bytesInOutput };
		}).sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
		return {
			path,
			kind: output.kind,
			rawBytes: bytes.length,
			gzipBytes: gzipSync(bytes, { level: 9 }).length,
			brotliBytes: brotliCompressSync(bytes, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }).length,
			entryPoint: meta.entryPoint ? portablePath(root, inputPath(meta.entryPoint)) : null,
			inputs,
			imports: meta.imports.map((entry) => ({ path: portablePath(root, resolve(outdir, entry.path)), kind: entry.kind }))
				.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0),
		};
	}));
	files.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
	return {
		schemaVersion: 1,
		bundler: `bun-${Bun.version}`,
		compression: { gzipLevel: 9, brotliQuality: 11, scope: "each-file" },
		files,
		totals: files.reduce((total, file) => ({
			rawBytes: total.rawBytes + file.rawBytes,
			gzipBytes: total.gzipBytes + file.gzipBytes,
			brotliBytes: total.brotliBytes + file.brotliBytes,
		}), { rawBytes: 0, gzipBytes: 0, brotliBytes: 0 }),
	};
}
