import { lstat, realpath } from "node:fs/promises";
import { isAbsolute, join, relative, sep } from "node:path";

export type Provider = "docker" | "podman";
export type Ownership = { kind: "run" | "dev"; runId: string; cluster: string; dir: string; raw: string; kubeconfig: string; dataDir: string; kindConfig: string; disableDefaultCNI: boolean; provider: Provider; kubernetes: string };

export function expectedCluster(kind: Ownership["kind"], id: string, workspaceHash: string) {
	return kind === "run" ? `kubecove-e2e-${id}` : `kubecove-dev-${workspaceHash}`;
}

export function contained(path: string, parent: string) {
	const value = relative(parent, path);
	return value === "" || (!value.startsWith("..") && !isAbsolute(value));
}

export function assertOwned(record: Ownership, kind: Ownership["kind"], dir: string, id: string, workspaceHash: string) {
	if (record.kind !== kind || record.runId !== id || record.cluster !== expectedCluster(kind, id, workspaceHash) || record.dir !== dir || ![record.raw, record.kubeconfig, record.dataDir, record.kindConfig].every((path) => contained(path, dir))) throw new Error("refuse operation outside exact ownership record");
}

export async function assertOwnedPathOnDisk(path: string, dir: string) {
	const ownedDir = await realpath(dir);
	if (!(await lstat(dir)).isDirectory()) throw new Error("refuse symlinked ownership path");
	let current = dir;
	for (const part of relative(dir, path).split(sep).filter(Boolean)) {
		current = join(current, part);
		try {
			if ((await lstat(current)).isSymbolicLink()) throw new Error("refuse symlinked ownership path");
		} catch (error) {
			if (error instanceof Error && error.message === "refuse symlinked ownership path") throw error;
			// SAFETY: Node filesystem errors expose `code`; only ENOENT permits a missing final path.
			if ((error as NodeJS.ErrnoException).code === "ENOENT") break;
			throw error;
		}
	}
	try {
		if (!contained(await realpath(path), ownedDir)) throw new Error("refuse operation outside real ownership directory");
	} catch (error) {
		if (error instanceof Error && error.message === "refuse operation outside real ownership directory") throw error;
		// SAFETY: Node filesystem errors expose `code`; any result besides ENOENT is unsafe to ignore.
		if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
	}
}

export async function assertOwnedOnDisk(record: Ownership, kind: Ownership["kind"], dir: string, id: string, workspaceHash: string) {
	assertOwned(record, kind, dir, id, workspaceHash);
	for (const path of [record.raw, record.kubeconfig, record.dataDir, record.kindConfig]) await assertOwnedPathOnDisk(path, dir);
}

export function ownershipFromDisk(stored: Partial<Ownership>, kind: Ownership["kind"], dir: string, id: string, workspaceHash: string, requireDefaultCni = kind === "dev") {
	// SAFETY: caller parses persisted ownership into this partial record, then `assertOwned` validates every required field.
	const record = { ...stored, kindConfig: stored.kindConfig ?? join(dir, "kind.yaml") } as Ownership;
	assertOwned(record, kind, dir, id, workspaceHash);
	if (requireDefaultCni && record.disableDefaultCNI !== true) throw new Error("legacy dev Kind ownership record lacks disableDefaultCNI proof; run bun run dev:kind:down before starting a new dev Kind lab");
	return record;
}
