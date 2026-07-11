import type {
	ArgoApplicationSourceSummary,
	ArgoApplicationSummary,
} from "@/lib/types";
import type { GitOpsSelection } from "./surfaceModel";

export type GitOpsSourceMode = "git" | "helm" | "multi" | "plugin" | "unknown";

export interface GitOpsTooltipField {
	label: string;
	value: string;
}

export interface GitOpsSourceTooltipRow {
	name: string;
	fields: GitOpsTooltipField[];
}

export interface GitOpsSourceTooltipGroup {
	label: string;
	rows: GitOpsSourceTooltipRow[];
}

export interface GitOpsRevisionTooltipRow {
	name: string;
	fields: GitOpsTooltipField[];
}

export function gitOpsSelectionSourceMode(
	selection: GitOpsSelection,
): GitOpsSourceMode | null {
	if (selection.type === "argoProject") return null;
	if (selection.type === "argoApp") {
		return selection.item.sourceMode ?? (selection.item.sourceRepo ? "git" : "unknown");
	}
	if (selection.type === "argoAppSet") {
		return selection.item.sourceRepo ? "git" : "unknown";
	}
	const sourceKind = selection.item.sourceKind ?? selection.item.resourceKind.kind;
	if (sourceKind.includes("Helm")) return "helm";
	if (sourceKind.includes("Git") || selection.item.resourceKind.kind === "Kustomization") {
		return "git";
	}
	return "unknown";
}

export function gitOpsSelectionSourceLabel(
	sourceMode: GitOpsSourceMode | null,
	sourceCount?: number,
): string {
	if (sourceMode === "git") return "Git source";
	if (sourceMode === "helm") return "Helm source";
	if (sourceMode === "multi") {
		return sourceCount ? `${sourceCount} sources` : "Multiple sources";
	}
	if (sourceMode === "plugin") return "Plugin source";
	if (sourceMode === "unknown") return "Unknown source";
	return "";
}

export function gitOpsSelectionSourceLine(selection: GitOpsSelection): string | null {
	if (selection.type === "argoApp") {
		return argoApplicationSourceLine(selection.item);
	}
	if (selection.type === "argoAppSet") {
		return selection.item.sourceRepo ?? null;
	}
	if (selection.type === "argoProject") {
		return selection.item.description ?? null;
	}
	const { item } = selection;
	if (item.sourceKind && item.sourceName) return `${item.sourceKind}/${item.sourceName}`;
	return item.message ?? null;
}

export function gitOpsSelectionRevisionLabel(selection: GitOpsSelection): string | null {
	if (selection.type === "argoApp") {
		return argoApplicationRevisionLabel(selection.item);
	}
	if (selection.type === "argoAppSet") return selection.item.sourceRevision;
	if (selection.type === "flux") return selection.item.lastAppliedRevision ?? null;
	return null;
}

export function gitOpsSelectionSourceTooltip(selection: GitOpsSelection): string {
	return [gitOpsSelectionSourceTooltipTitle(selection), ...gitOpsSelectionSourceTooltipLines(selection)]
		.filter(Boolean)
		.join("\n");
}

export function gitOpsSelectionSourceTooltipTitle(selection: GitOpsSelection): string {
	const total = gitOpsSelectionSourceTooltipGroups(selection).reduce(
		(count, group) => count + group.rows.length,
		0,
	);
	if (total > 0) return `Sources · ${total} total`;
	return gitOpsSelectionSourceLine(selection) ? "Sources · 1 total" : "Sources";
}

export function gitOpsSelectionSourceTooltipGroups(
	selection: GitOpsSelection,
): GitOpsSourceTooltipGroup[] {
	if (selection.type !== "argoApp") return [];
	const sources = selection.item.sources ?? [];
	const groups = new Map<string, GitOpsSourceTooltipRow[]>();
	for (const source of sources) {
		const mode = argoSourceModeLabel(source.sourceMode);
		const rows = groups.get(mode) ?? [];
		rows.push({
			name: argoSourceCompactName(source) ?? "Source",
			fields: argoSourceTooltipFields(source),
		});
		groups.set(mode, rows);
	}
	return [...groups.entries()].map(([label, rows]) => ({ label, rows }));
}

export function gitOpsSelectionSourceTooltipLines(selection: GitOpsSelection): string[] {
	const groups = gitOpsSelectionSourceTooltipGroups(selection);
	if (groups.length > 0) {
		return groups.flatMap((group) =>
			group.rows.map((row) => {
				const fields = row.fields.map((field) => `${field.label} ${field.value}`).join(" · ");
				return `${group.label}: ${row.name}${fields ? ` · ${fields}` : ""}`;
			}),
		);
	}
	const line = gitOpsSelectionSourceLine(selection);
	return line ? [line] : [];
}

export function gitOpsSelectionRevisionTooltipTitle(selection: GitOpsSelection): string {
	return `Revisions · ${gitOpsSelectionRevisionTooltipRows(selection).length} unique`;
}

export function gitOpsSelectionRevisionTooltipRows(
	selection: GitOpsSelection,
): GitOpsRevisionTooltipRow[] {
	if (selection.type === "argoApp") {
		return argoRevisionTooltipRows(selection.item);
	}
	if (selection.type === "argoAppSet" && selection.item.sourceRevision) {
		return [
			{
				name: selection.item.name,
				fields: [{ label: "revision", value: selection.item.sourceRevision }],
			},
		];
	}
	if (selection.type === "flux" && selection.item.lastAppliedRevision) {
		return [
			{
				name: selection.item.name,
				fields: [{ label: "applied", value: selection.item.lastAppliedRevision }],
			},
		];
	}
	return [];
}

export function gitOpsSelectionRevisionTooltipLines(selection: GitOpsSelection): string[] {
	return gitOpsSelectionRevisionTooltipRows(selection).map((row) => {
		const fields = row.fields.map((field) => `${field.label} ${field.value}`).join(" · ");
		return `${row.name}: ${fields}`;
	});
}

export function gitOpsSelectionAgeTooltip(selection: GitOpsSelection): string | null {
	return selection.item.createdAt ?? null;
}

function argoApplicationSourceLine(app: ArgoApplicationSummary): string | null {
	const sources = app.sources ?? [];
	if (sources.length === 0) return app.sourceRepo ?? null;
	if (sources.length === 1) return argoSourceLine(sources[0]);
	const names = [
		...new Set(sources.map(argoSourceCompactName).filter((name): name is string => Boolean(name))),
	];
	if (names.length === 0) return `${sources.length} sources`;
	const visible = names.slice(0, 2).join(" · ");
	return `${sources.length} sources · ${visible}${names.length > 2 ? " · ..." : ""}`;
}

function argoApplicationRevisionLabel(app: ArgoApplicationSummary): string | null {
	const rows = argoRevisionTooltipRows(app);
	if (rows.length === 0) return app.sourceRevision;
	if (rows.length === 1) return rows[0]?.fields[0]?.value ?? null;
	return `${rows.length} revisions`;
}

function argoSourceLine(source: ArgoApplicationSourceSummary): string | null {
	if (source.chart && source.repoUrl) return `${source.chart} · ${source.repoUrl}`;
	if (source.repoUrl && source.path) return `${source.repoUrl}/${source.path}`;
	return source.repoUrl ?? source.chart ?? source.path ?? source.reference ?? null;
}

function argoSourceCompactName(source: ArgoApplicationSourceSummary): string | null {
	return source.chart ?? source.path ?? shortRepositoryName(source.repoUrl) ?? source.reference ?? null;
}

function argoSourceTooltipFields(source: ArgoApplicationSourceSummary): GitOpsTooltipField[] {
	return [
		source.repoUrl ? { label: "repo", value: source.repoUrl } : null,
		source.path ? { label: "path", value: source.path } : null,
		source.chart ? { label: "chart", value: source.chart } : null,
		source.reference ? { label: "ref", value: source.reference } : null,
		source.targetRevision ? { label: "target", value: source.targetRevision } : null,
		source.resolvedRevision ? { label: "resolved", value: source.resolvedRevision } : null,
	].filter((field): field is GitOpsTooltipField => Boolean(field));
}

function argoRevisionTooltipRows(app: ArgoApplicationSummary): GitOpsRevisionTooltipRow[] {
	const sources = app.sources ?? [];
	if (sources.length === 0) {
		return app.sourceRevision
			? [{ name: app.name, fields: [{ label: "revision", value: app.sourceRevision }] }]
			: [];
	}
	const revisions = new Map<
		string,
		{ names: string[]; targetRevision: string | null; resolvedRevision: string | null }
	>();
	for (const [index, source] of sources.entries()) {
		const targetRevision = source.targetRevision ?? null;
		const resolvedRevision = source.resolvedRevision ?? null;
		if (!targetRevision && !resolvedRevision) continue;
		const key = `${targetRevision ?? ""}\u0000${resolvedRevision ?? ""}`;
		const revision = revisions.get(key) ?? {
			names: [],
			targetRevision,
			resolvedRevision,
		};
		const name = argoSourceCompactName(source) ?? `Source ${index + 1}`;
		if (!revision.names.includes(name)) revision.names.push(name);
		revisions.set(key, revision);
	}
	return [...revisions.values()].map((revision) => {
		const fields = [
			revision.targetRevision ? { label: "target", value: revision.targetRevision } : null,
			revision.resolvedRevision
				? { label: "resolved", value: shortGitSha(revision.resolvedRevision) }
				: null,
		].filter((field): field is GitOpsTooltipField => Boolean(field));
		return {
			name: compactNameList(revision.names),
			fields,
		};
	});
}

function argoSourceModeLabel(mode: ArgoApplicationSourceSummary["sourceMode"]): string {
	if (mode === "git") return "Git";
	if (mode === "helm") return "Helm";
	if (mode === "plugin") return "Plugin";
	return "Unknown";
}

function compactNameList(names: string[]): string {
	if (names.length === 0) return "Source";
	if (names.length <= 2) return names.join(", ");
	return `${names.slice(0, 2).join(", ")} +${names.length - 2}`;
}

function shortGitSha(revision: string): string {
	return /^[0-9a-f]{8,}$/i.test(revision) ? revision.slice(0, 7) : revision;
}

function shortRepositoryName(repoUrl: string | null): string | null {
	if (!repoUrl) return null;
	const trimmed = repoUrl.replace(/\/$/, "");
	const lastPart = trimmed.split("/").pop();
	return lastPart?.replace(/\.git$/, "") ?? repoUrl;
}
