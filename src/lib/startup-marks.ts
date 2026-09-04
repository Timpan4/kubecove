export const STARTUP_MILESTONES = [
	"frontend-entry",
	"svelte-mount",
	"path-restored",
	"launcher-ready",
	"workspace-ready",
	"kubeconfig-ready",
	"base-scope-ready",
	"first-resource-rows",
] as const;

export type StartupMilestone = (typeof STARTUP_MILESTONES)[number];
export const STARTUP_PREFIX = "kubecove:startup:";
export const IPC_PREFIX = "kubecove:ipc:";
export const MAX_PROFILE_IPC_ENTRIES = 1_000;
const marked = new Set<StartupMilestone>();
let ipcEntries = 0;

export function markStartup(milestone: StartupMilestone): void {
	if (marked.has(milestone)) return;
	marked.add(milestone);
	performance.mark(`${STARTUP_PREFIX}${milestone}`);
}

export function markProfileIpc(command: string, start: number, succeeded: boolean): void {
	if (process.env.KUBECOVE_PUBLIC_PROFILE !== "true") return;
	if (ipcEntries >= MAX_PROFILE_IPC_ENTRIES) return;
	// Only command names cross into the profiler; never arguments, results, or errors.
	if (!/^[a-z][a-z0-9_]{0,100}$/.test(command)) return;
	ipcEntries += 1;
	performance.measure(`${IPC_PREFIX}${command}:${succeeded ? "ok" : "error"}`, {
		start,
		end: performance.now(),
	});
}

markStartup("frontend-entry");
