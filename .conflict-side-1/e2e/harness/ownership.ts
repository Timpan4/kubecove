import { isAbsolute, join, relative } from "node:path";

export type Provider = "docker" | "podman";
export type Ownership = { kind: "run" | "dev"; runId: string; cluster: string; dir: string; raw: string; kubeconfig: string; dataDir: string; kindConfig: string; provider: Provider; kubernetes: string };

export function expectedCluster(kind: Ownership["kind"], id: string, workspaceHash: string) {
	return kind === "run" ? `kubecove-e2e-${id}` : `kubecove-dev-${workspaceHash}`;
}

export function contained(path: unknown, parent: string) {
	if (typeof path !== "string") return false;
	const value = relative(parent, path);
	return value === "" || (!value.startsWith("..") && !isAbsolute(value));
}

export function assertOwned(record: Ownership, kind: Ownership["kind"], dir: string, id: string, workspaceHash: string) {
	if (record.kind !== kind || record.runId !== id || record.cluster !== expectedCluster(kind, id, workspaceHash) || record.dir !== dir || ![record.raw, record.kubeconfig, record.dataDir, record.kindConfig].every((path) => contained(path, dir))) throw new Error("refuse operation outside exact ownership record");
}

export function ownershipFromDisk(value: unknown, kind: Ownership["kind"], dir: string, id: string, workspaceHash: string) {
	const stored = value && typeof value === "object" ? value as Partial<Ownership> : {};
	const record = { ...stored, kindConfig: stored.kindConfig ?? join(dir, "kind.yaml") } as Ownership;
	assertOwned(record, kind, dir, id, workspaceHash);
	return record;
}
