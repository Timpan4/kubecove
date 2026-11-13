import type { Snippet } from "svelte";
import type { IncidentFilter } from "@/features/incidents";
import type { HealthFilter } from "@/features/resources/helpers";
import type { ResourceSummary } from "@/lib/types";
import type { WorkspaceEntryPoint } from "@/lib/workspace-entry-points";
import type { WorkspaceHealthSummary, WorkspaceShortcut } from "@/lib/workspaces";
import type { OverviewModel, ProblemTone, Tone } from "./model";
import type { EntryRow } from "./entries";

export interface OverviewView {
    workspaceId: string;
	model: OverviewModel;
	health: WorkspaceHealthSummary;
	/** False until a resource load has succeeded, so no count or verdict is shown before then. */
	hasData: boolean;
	/** True when the load failed or skipped part of the scope. */
	partial: boolean;
	multiContext: boolean;
	shortcuts: WorkspaceShortcut[];
	portForwardCount: number;
	pinned: EntryRow[];
	recent: EntryRow[];
	primaryContext: string;
	/** The current page's own load, retry and scope-warning block. */
	loadState: Snippet;
	/** The current page's own GitOps inventory block, with its error states. */
	gitOps: Snippet;
	onOpenResources: (namespace?: string, health?: HealthFilter) => void;
	onOpenResource: (resource: ResourceSummary) => void;
	onOpenEntry: (entry: WorkspaceEntryPoint) => void;
	onOpenShortcut: (shortcut: WorkspaceShortcut) => void;
	onOpenIncidents: (filter?: IncidentFilter) => void;
	onOpenApp: (resource: ResourceSummary) => void;
	onOpenPortForwards: () => void;
}

export const DOT = {
	degraded: "bg-red-400",
	needsAttention: "bg-amber-400",
	healthy: "bg-emerald-400",
	unknown: "bg-muted-foreground/60",
	notEvaluated: "bg-muted-foreground/30",
} satisfies Record<Tone, string>;
export const HOLLOW_DOT = "border border-muted-foreground/50";
export const TONE_LABEL = {
	degraded: "Degraded",
	needsAttention: "Needs attention",
	healthy: "Healthy",
	unknown: "Unknown",
	notEvaluated: "Not evaluated",
} satisfies Record<Tone, string>;
export const STATUS_BADGE = {
	degraded: "border-red-500/40 text-red-300",
	needsAttention: "border-amber-500/40 text-amber-300",
} satisfies Record<ProblemTone, string>;
// The link variant's primary color is too dim on a card, so names use the foreground color.
export const LINK =
	"h-auto max-w-full justify-start p-0 text-left font-mono break-all whitespace-normal text-foreground underline decoration-muted-foreground/40 hover:decoration-foreground";
export const ROW = "h-auto w-full justify-start gap-2 px-2 py-1.5 text-left whitespace-normal";

export const plural = (count: number, noun: string) => `${count} ${noun}${count === 1 ? "" : "s"}`;
