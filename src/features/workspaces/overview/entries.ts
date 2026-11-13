import { resourceKey } from "@/lib/resource-identity";
import type { ResourceSummary } from "@/lib/types";
import { entryPointKey, resourceFromEntryPoint, type WorkspaceEntryPoint } from "@/lib/workspace-entry-points";
import { toneOf, type OverviewModel, type Tone } from "./model";

export interface EntryRow {
	key: string; entry: WorkspaceEntryPoint; kind: string; name: string; namespace?: string; tone: Tone | null;
}
function entryKind(entry: WorkspaceEntryPoint): string {
	if (entry.kind === "namespace") return "Namespace";
	if (entry.kind === "app") return "Application";
	return entry.resourceKind ?? "Resource";
}
export function entryRows(entries: WorkspaceEntryPoint[], rows: readonly ResourceSummary[], model: OverviewModel): EntryRow[] {
	const resources = new Map(rows.map(row => [resourceKey(row), row]));
	return entries.map(entry => {
		let tone: Tone | null = null;
		if (entry.kind === "namespace") {
			// Namespace findings span the same workspace contexts as their navigation target.
			const namespace = model.namespaces.find(row => row.namespace === (entry.namespace ?? entry.name));
			if (namespace?.degraded) tone = "degraded";
			else if (namespace?.attention) tone = "needsAttention";
		} else {
			const target = resourceFromEntryPoint(entry.kind === "app" ? { ...entry, kind: "resource", resourceKind: "Application" } : entry);
			const row = target && resources.get(resourceKey(target));
			if (row) tone = toneOf(row);
		}
		return { key: entryPointKey(entry), entry, kind: entryKind(entry), name: entry.name, namespace: entry.namespace, tone };
	});
}
export function entryLabel(entry: WorkspaceEntryPoint): string {
	return `${entryKind(entry)} / ${entry.name}`;
}
