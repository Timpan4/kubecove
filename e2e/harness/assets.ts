import { createHash } from "node:crypto";

const DOWNLOAD_ATTEMPTS = 3;

export async function downloadAsset(
	name: string,
	url: string,
	fetcher: (url: string) => Promise<Response> = fetch,
): Promise<Uint8Array> {
	let lastError: unknown;
	for (let attempt = 1; attempt <= DOWNLOAD_ATTEMPTS; attempt += 1) {
		let response: Response;
		try {
			response = await fetcher(url);
			if (response.ok) return new Uint8Array(await response.arrayBuffer());
		} catch (error) {
			lastError = error;
			continue;
		}
		const error = new Error(`download failed: ${name} (${response.status})`);
		if (response.status !== 429 && response.status < 500) throw error;
		lastError = error;
	}
	throw lastError instanceof Error ? lastError : new Error(`download failed: ${name}`);
}

export function sha256(bytes: Uint8Array) {
	return createHash("sha256").update(bytes).digest("hex");
}

export function verifyAsset(name: string, bytes: Uint8Array, expected: string) {
	const actual = sha256(bytes);
	if (actual !== expected) throw new Error(`checksum mismatch: ${name}`);
	return actual;
}

