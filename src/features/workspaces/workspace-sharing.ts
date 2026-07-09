import {
	createSavedPortForward,
	createWorkspaceRecord,
	workspaceScopeContexts,
	type SavePortForwardInput,
	type SavedWorkspace,
} from "@/lib/workspace-model";
import { cloneScope, cloneShortcut, parseWorkspaceImport } from "./workspace-sharing-parse";
import {
	WORKSPACE_EXPORT_API_VERSION,
	type SharedWorkspaceDocument,
	type SharedWorkspaceListDocument,
	type WorkspaceImportDecisions,
	type WorkspaceImportItem,
	type WorkspaceImportPreview,
	type WorkspaceImportResult,
} from "./workspace-sharing-schema";

export { WORKSPACE_EXPORT_API_VERSION } from "./workspace-sharing-schema";
export type {
	SharedWorkspaceDocument,
	SharedWorkspaceListDocument,
	WorkspaceImportAction,
	WorkspaceImportDecisions,
	WorkspaceImportItem,
	WorkspaceImportPreview,
	WorkspaceImportResult,
} from "./workspace-sharing-schema";

export function slugifyWorkspaceName(name: string): string {
	const slug = name
		.trim()
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, "-")
		.replace(/^-+|-+$/g, "");
	return slug || "workspace";
}

export function serializeWorkspaceExport(workspaces: SavedWorkspace[]): string {
	const names = uniqueExportNames(workspaces);
	const items = workspaces.map((workspace, index) =>
		workspaceExportDocument(workspace, names[index]),
	);
	const document =
		items.length === 1
			? items[0]
			: ({
					apiVersion: WORKSPACE_EXPORT_API_VERSION,
					kind: "WorkspaceList",
					metadata: { name: "kubecove-workspaces" },
					items,
				} satisfies SharedWorkspaceListDocument);
	return `${JSON.stringify(document, null, 2)}\n`;
}

export function buildWorkspaceImportPreview(
	raw: string,
	existing: SavedWorkspace[],
): WorkspaceImportPreview {
	return {
		items: parseWorkspaceImport(raw).map((workspace, index) => {
			const match = findExistingWorkspace(workspace, existing);
			return {
				id: `${workspace.sharedKey}:${index}`,
				workspace,
				existingWorkspaceId: match?.id,
				existingWorkspaceName: match?.name,
				defaultAction: match ? "skip" : "add",
			};
		}),
	};
}

export function applyWorkspaceImport(
	existing: SavedWorkspace[],
	preview: WorkspaceImportPreview,
	decisions: WorkspaceImportDecisions,
	now = new Date().toISOString(),
): WorkspaceImportResult {
	let added = 0;
	let replaced = 0;
	let skipped = 0;
	const retained = [...existing];
	const additions: SavedWorkspace[] = [];

	for (const item of preview.items) {
		const action = decisions[item.id] ?? item.defaultAction;
		if (action === "skip") {
			skipped += 1;
			continue;
		}
		const matchIndex = retained.findIndex(
			(workspace) => workspace.id === item.existingWorkspaceId,
		);
		if (action === "replace" && matchIndex >= 0) {
			retained[matchIndex] = sharedToSavedWorkspace(
				item.workspace,
				now,
				retained[matchIndex],
				retained.filter((_, index) => index !== matchIndex).concat(additions),
			);
			replaced += 1;
			continue;
		}
		additions.push(sharedToSavedWorkspace(item.workspace, now, undefined, retained.concat(additions)));
		added += 1;
	}

	return { workspaces: [...additions, ...retained], added, replaced, skipped };
}

function workspaceExportDocument(
	workspace: SavedWorkspace,
	metadataName: string,
): SharedWorkspaceDocument {
	return {
		apiVersion: WORKSPACE_EXPORT_API_VERSION,
		kind: "Workspace",
		metadata: { name: metadataName },
		spec: {
			displayName: workspace.name,
			scope: cloneScope(workspace.scope),
			shortcuts: workspace.shortcuts.map(cloneShortcut),
			portForwards: (workspace.portForwards ?? []).map((forward) => ({
				clusterContext: forward.clusterContext,
				namespace: forward.namespace,
				serviceName: forward.serviceName,
				servicePort: forward.servicePort,
				localPort: forward.localPort,
				label: forward.label,
			})),
		},
	};
}

function uniqueExportNames(workspaces: SavedWorkspace[]): string[] {
	const used = new Set<string>();
	return workspaces.map((workspace) => {
		const base = slugifyWorkspaceName(workspace.sharedKey ?? workspace.name);
		return uniqueKey(base, used);
	});
}

function sharedToSavedWorkspace(
	workspace: WorkspaceImportItem["workspace"],
	now: string,
	replace?: SavedWorkspace,
	others: SavedWorkspace[] = [],
): SavedWorkspace {
	const name = replace ? workspace.name : uniqueName(workspace.name, others);
	const sharedKey = replace ? workspace.sharedKey : uniqueKey(workspace.sharedKey, new Set(others.map((item) => item.sharedKey).filter(Boolean) as string[]));
	const scope = cloneScope(workspace.scope);
	const base = createWorkspaceRecord(
		{
			name,
			clusterContext: scope.clusterContext,
			clusterContexts: workspaceScopeContexts(scope),
			clusterGroupName: scope.clusterGroup?.name,
			namespaces: scope.namespaces,
			kinds: scope.kinds,
			shortcutPreferences: scope.shortcutPreferences,
		},
		now,
	);
	return {
		...base,
		id: replace?.id ?? base.id,
		sharedKey,
		name,
		createdAt: replace?.createdAt ?? now,
		updatedAt: now,
		scope,
		shortcuts: workspace.shortcuts.map(cloneShortcut),
		portForwards: workspace.portForwards.map((forward) =>
			createSavedPortForward(forward satisfies SavePortForwardInput, now),
		),
	};
}

function findExistingWorkspace(
	workspace: WorkspaceImportItem["workspace"],
	existing: SavedWorkspace[],
): SavedWorkspace | undefined {
	return (
		existing.find((item) => item.sharedKey === workspace.sharedKey) ??
		existing.find((item) => item.name === workspace.name)
	);
}

function uniqueName(name: string, existing: SavedWorkspace[]): string {
	const used = new Set(existing.map((workspace) => workspace.name));
	if (!used.has(name)) return name;
	let index = 2;
	while (used.has(`${name} (${index})`)) index += 1;
	return `${name} (${index})`;
}

function uniqueKey(base: string, used: Set<string>): string {
	let key = slugifyWorkspaceName(base);
	let index = 2;
	while (used.has(key)) {
		key = `${slugifyWorkspaceName(base)}-${index}`;
		index += 1;
	}
	used.add(key);
	return key;
}
