import type { HealthAssessmentState } from "@/lib/health-types";
import { formatStatusLabel } from "@/lib/utils";
import { type GitOpsSelection, gitOpsSelectionKey } from "./surfaceModel";
import type { GitOpsTrackedRequest } from "./surfacePresentation";

/** The GitOps list groups rows by health so resources that need a look sit on top. */
export type GitOpsListGroup = "attention" | "healthy" | "notEvaluated";

export type GitOpsListEntry =
	| { kind: "group"; group: GitOpsListGroup; position: number; label: string; count: number; collapsed: boolean; last: boolean }
	/** `expanded` rows add a second line with the assessment reasons and raw statuses. */
	| { kind: "row"; selection: GitOpsSelection; expanded: boolean; last: boolean };

export type GitOpsListSyncTone = "quiet" | "active" | "attention" | "failed";

export interface GitOpsListSync {
	label: string;
	tone: GitOpsListSyncTone;
}

export interface GitOpsListColumns {
	sync: string;
	destination: string;
}

interface GitOpsListGroupRows {
	attention: GitOpsSelection[];
	healthy: GitOpsSelection[];
	notEvaluated: GitOpsSelection[];
}

const GROUP_ORDER: GitOpsListGroup[] = ["attention", "healthy", "notEvaluated"];
const GROUP_LABELS = {
	attention: "Not healthy",
	healthy: "Healthy",
	notEvaluated: "Not evaluated",
} satisfies Record<GitOpsListGroup, string>;
/** Order inside the attention group: the worst state first. */
const ATTENTION_RANK = {
	degraded: 0,
	needsAttention: 1,
	unknown: 2,
	healthy: 3,
	notEvaluated: 3,
} satisfies Record<HealthAssessmentState, number>;

export function gitOpsListState(selection: GitOpsSelection): HealthAssessmentState {
	return selection.item.healthAssessment?.state ?? "notEvaluated";
}

function groupOf(state: HealthAssessmentState): GitOpsListGroup {
	if (state === "healthy") return "healthy";
	if (state === "notEvaluated") return "notEvaluated";
	return "attention";
}

export function buildGitOpsListEntries(
	selections: GitOpsSelection[],
	collapsed: ReadonlySet<GitOpsListGroup>,
): GitOpsListEntry[] {
	const groups: GitOpsListGroupRows = { attention: [], healthy: [], notEvaluated: [] };
	for (const selection of selections) groups[groupOf(gitOpsListState(selection))].push(selection);
	groups.attention.sort(
		(left, right) => ATTENTION_RANK[gitOpsListState(left)] - ATTENTION_RANK[gitOpsListState(right)],
	);

	const entries: GitOpsListEntry[] = [];
	let position = 0;
	for (const group of GROUP_ORDER) {
		const rows = groups[group];
		if (rows.length === 0) continue;
		const isCollapsed = collapsed.has(group);
		entries.push({ kind: "group", group, position: position++, label: GROUP_LABELS[group], count: rows.length, collapsed: isCollapsed, last: false });
		if (isCollapsed) continue;
		for (const selection of rows) {
			entries.push({ kind: "row", selection, expanded: group === "attention", last: false });
		}
	}
	const last = entries[entries.length - 1];
	if (last) last.last = true;
	return entries;
}

export function gitOpsListEntryKey(entry: GitOpsListEntry): string {
	return entry.kind === "group" ? `group:${entry.group}` : gitOpsSelectionKey(entry.selection);
}

/** Column labels that depend on what the list holds. Every row of one list has the same type. */
export function gitOpsListColumns(selection: GitOpsSelection | undefined): GitOpsListColumns {
	if (selection?.type === "flux") return { sync: "Ready", destination: "Namespace" };
	if (selection?.type === "argoProject") return { sync: "Status", destination: "Namespace" };
	return { sync: "Sync", destination: "Destination" };
}

/**
 * One word for the delivery state of a row. For an Application the most urgent fact wins:
 * a request KubeCove sent that Argo CD has not settled, then a running or failed operation,
 * then the sync status. A settled request stays in the row's Last request status.
 */
export function gitOpsListSync(selection: GitOpsSelection, request: GitOpsTrackedRequest | null): GitOpsListSync {
	if (selection.type === "flux") {
		const ready = selection.item.readyStatus;
		if (!ready) return { label: "-", tone: "quiet" };
		if (ready === "True") return { label: "Ready", tone: "quiet" };
		if (ready === "False") return { label: "Not ready", tone: "failed" };
		return { label: formatStatusLabel(ready), tone: "attention" };
	}
	if (selection.type === "argoProject") {
		return { label: selection.item.status ? formatStatusLabel(selection.item.status) : "-", tone: "quiet" };
	}
	if (request?.busy) return { label: request.label, tone: "active" };
	if (selection.type === "argoApp") {
		if (selection.item.refreshRequested) return { label: "Refreshing", tone: "active" };
		const phase = selection.item.operationPhase;
		if (phase === "Running") return { label: "Syncing", tone: "active" };
		if (phase === "Terminating") return { label: "Terminating", tone: "active" };
		if (phase === "Failed" || phase === "Error") return { label: "Sync failed", tone: "failed" };
	}
	const sync = selection.item.syncStatus;
	if (!sync) return { label: "-", tone: "quiet" };
	if (sync === "Synced") return { label: "Synced", tone: "quiet" };
	if (sync === "OutOfSync") return { label: "Out of sync", tone: "attention" };
	return { label: formatStatusLabel(sync), tone: "attention" };
}
