import { IPC_PREFIX, MAX_PROFILE_IPC_ENTRIES, STARTUP_MILESTONES, STARTUP_PREFIX } from "../../src/lib/startup-marks";
import type { AppUsageMetrics } from "../../src/lib/types";

export interface ProfileEntry {
	name: string;
	startTime: number;
	duration: number;
}

export function startupReport(entries: ProfileEntry[], heapBytes: number | null, usage: AppUsageMetrics | null) {
	const milestones = STARTUP_MILESTONES.map((name) => {
		const entry = entries.find((entry) => entry.name === `${STARTUP_PREFIX}${name}`);
		return { name, atMs: entry?.startTime ?? null, unavailable: entry ? null : "not-reached" };
	}).sort((a, b) => (a.atMs ?? Infinity) - (b.atMs ?? Infinity));
	const groups = new Map<string, { command: string; count: number; errors: number; totalMs: number; maxMs: number }>();
	let measuredCalls = 0;
	for (const entry of entries) {
		if (!entry.name.startsWith(IPC_PREFIX)) continue;
		const match = /^([a-z][a-z0-9_]{0,100}):(ok|error)$/.exec(entry.name.slice(IPC_PREFIX.length));
		if (!match || measuredCalls >= MAX_PROFILE_IPC_ENTRIES) continue;
		measuredCalls += 1;
		const command = match[1];
		const group = groups.get(command) ?? { command, count: 0, errors: 0, totalMs: 0, maxMs: 0 };
		group.count += 1;
		group.errors += Number(match[2] === "error");
		group.totalMs += entry.duration;
		group.maxMs = Math.max(group.maxMs, entry.duration);
		groups.set(command, group);
	}
	const memoryGroup = (label: string) => {
		const group = usage?.breakdown.find((group) => group.label === label);
		return {
			bytes: group?.memoryBytes ?? null,
			processCount: group?.processCount ?? null,
			unavailable: group ? null : "not-observed",
		};
	};
	return {
		milestones,
		ipc: {
			measurement: "frontend-invoke-to-settlement",
			limit: MAX_PROFILE_IPC_ENTRIES,
			limitReached: measuredCalls === MAX_PROFILE_IPC_ENTRIES,
			commands: [...groups.values()].sort((a, b) => a.command.localeCompare(b.command)),
		},
		memory: {
			host: memoryGroup("KubeCove"),
			webview: memoryGroup("WebView"),
			otherChildren: memoryGroup("Other children"),
			processTreeBytes: usage?.memoryBytes ?? null,
			jsHeap: { bytes: heapBytes, unavailable: heapBytes === null ? "engine-does-not-expose-heap" : null },
		},
	};
}
