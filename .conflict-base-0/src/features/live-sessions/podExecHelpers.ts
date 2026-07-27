import { normalizeKubeconfigEnvVar } from "@/lib/settings";
import type {
	PodExecSessionRequest,
	PodExecSessionSummary,
	ResourceSummary,
} from "@/lib/types";

export type PodExecPreset = "sh" | "bash" | "custom";

export interface PodExecDraft {
	preset: PodExecPreset;
	customArgv: string;
	container?: string;
	cols: number;
	rows: number;
	confirmed: boolean;
}

export function podExecTarget(resource: ResourceSummary, container?: string): string {
	const containerName = container?.trim() || "<default>";
	return `${resource.cluster}/${resource.namespace ?? ""}/Pod/${resource.name}/container/${containerName}`;
}

export function podExecCommandText(command: string[]): string {
	return JSON.stringify(command);
}

export function commandForPreset(
	preset: PodExecPreset,
	customArgv: string,
): string[] | string {
	if (preset === "sh") return ["/bin/sh"];
	if (preset === "bash") return ["/bin/bash"];
	const command = customArgv
		.split(/\r?\n/)
		.map((part) => part.trim())
		.filter(Boolean);
	return command.length > 0 ? command : "Custom argv is required";
}

export function buildPodExecRequest(
	resource: ResourceSummary,
	draft: PodExecDraft,
	kubeconfigSource?: string,
): PodExecSessionRequest | string {
	if (resource.kind !== "Pod") return "Pod exec starts from an exact Pod";
	if (!resource.namespace) return "Pod exec requires a namespace";
	const command = commandForPreset(draft.preset, draft.customArgv);
	if (typeof command === "string") return command;
	if (draft.cols < 1 || draft.rows < 1 || draft.cols > 500 || draft.rows > 500) {
		return "Terminal size must be between 1 and 500 columns and rows";
	}
	if (!draft.confirmed) return "Confirm the exact target and command before starting exec";

	return {
		clusterContext: resource.cluster,
		kubeconfigEnvVar: podExecRequestKubeconfigEnvVar(kubeconfigSource),
		namespace: resource.namespace,
		podName: resource.name,
		container: draft.container || undefined,
		command,
		stdin: true,
		tty: true,
		terminalSize: { cols: draft.cols, rows: draft.rows },
		confirmation: {
			acknowledged: draft.confirmed,
			target: podExecTarget(resource, draft.container),
			command: podExecCommandText(command),
		},
	};
}

export function isPodExecForResource(
	session: PodExecSessionSummary,
	resource: ResourceSummary,
	kubeconfigSource?: string,
): boolean {
	return (
		resource.kind === "Pod" &&
		podExecSessionMatchesKubeconfigSource(session, kubeconfigSource) &&
		session.clusterContext === resource.cluster &&
		session.namespace === resource.namespace &&
		session.podName === resource.name
	);
}

export function podExecSessionMatchesKubeconfigSource(
	session: PodExecSessionSummary,
	kubeconfigSource?: string,
): boolean {
	const source = kubeconfigSource?.startsWith("kubeconfigSource=")
		? kubeconfigSource
		: `kubeconfigEnv=${normalizeKubeconfigEnvVar(kubeconfigSource)}`;
	const sessionSource =
		session.kubeconfigSourceKey ??
		`kubeconfigEnv=${normalizeKubeconfigEnvVar(session.kubeconfigEnvVar)}`;
	return sessionSource === source;
}

export function podExecRequestKubeconfigEnvVar(
	source?: string,
): string | undefined {
	return source?.startsWith("kubeconfigSource=") ? undefined : source;
}

export function sortPodExecSessions(
	sessions: PodExecSessionSummary[],
): PodExecSessionSummary[] {
	return [...sessions].sort((a, b) => a.startedAt.localeCompare(b.startedAt));
}
