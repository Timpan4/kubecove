import { createHash } from "node:crypto";

export function sha256(bytes: Uint8Array) {
	return createHash("sha256").update(bytes).digest("hex");
}

export function verifyAsset(name: string, bytes: Uint8Array, expected: string) {
	const actual = sha256(bytes);
	if (actual !== expected) throw new Error(`checksum mismatch: ${name}`);
	return actual;
}

