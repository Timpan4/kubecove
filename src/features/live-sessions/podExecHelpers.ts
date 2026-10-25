import { normalizeKubeconfigEnvVar } from "@/lib/settings";
import type {
	PodExecSessionRequest,
	PodExecSessionSummary,
	ResourceSummary,
} from "@/lib/types";

export const POD_EXEC_SHELLS = [["/bin/sh"], ["/bin/bash"]] as const;

export interface PodExecDraft {
	command: readonly string[];
	container?: string;
	cols: number;
	rows: number;
}

export function podExecTarget(resource: ResourceSummary, container?: string): string {
	const containerName = container?.trim() || "<default>";
	return `${resource.cluster}/${resource.namespace ?? ""}/Pod/${resource.name}/container/${containerName}`;
}

export function podExecCommandText(command: readonly string[]): string {
	return JSON.stringify(command);
}

export function podExecCommandLabel(command: readonly string[]): string {
	return command.join(" ");
}

export function parsePodExecArgv(text: string): string[] {
	return text
		.split(/\r?\n/)
		.map((part) => part.trim())
		.filter(Boolean);
}

export type PodExecRequestResult =
	| { ok: true; request: PodExecSessionRequest }
	| { ok: false; message: string };

function invalid(message: string): PodExecRequestResult {
	return { ok: false, message };
}

export function buildPodExecRequest(
	resource: ResourceSummary,
	draft: PodExecDraft,
	kubeconfigSource?: string,
): PodExecRequestResult {
	if (resource.kind !== "Pod") return invalid("Pod exec starts from an exact Pod");
	if (!resource.namespace) return invalid("Pod exec requires a namespace");
	const command = [...draft.command];
	if (command.length === 0) return invalid("Enter a command to run");
	if (draft.cols < 1 || draft.rows < 1 || draft.cols > 500 || draft.rows > 500) {
		return invalid("Terminal size must be between 1 and 500 columns and rows");
	}

	// Exec starts only from an action labeled with this exact command and container,
	// which is the explicit confirmation ADR 0005 requires.
	const request: PodExecSessionRequest = {
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
			acknowledged: true,
			target: podExecTarget(resource, draft.container),
			command: podExecCommandText(command),
		},
	};
	return { ok: true, request };
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
