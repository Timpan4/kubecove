import type {
	WorkspaceCompareEntry,
	WorkspaceScope,
	WorkspaceShortcut,
	WorkspaceShortcutPreferences,
} from "./workspace-model";
import type { DiscoveredResourceKind, ResourceKindSelection } from "./types";
import {
	WORKSPACE_EXPORT_API_VERSION,
	type SharedWorkspacePortForward,
	type WorkspaceImportItem,
} from "./workspace-sharing-schema";

export function parseWorkspaceImport(raw: string): WorkspaceImportItem["workspace"][] {
	let parsed: unknown;
	try {
		parsed = JSON.parse(raw);
	} catch {
		throw new Error("Workspace import must be valid JSON.");
	}
	const value = object(parsed, "Workspace import");
	if (value.apiVersion !== WORKSPACE_EXPORT_API_VERSION) {
		throw new Error(`Unsupported workspace export version: ${String(value.apiVersion)}`);
	}
	if (value.kind === "Workspace") return [parseWorkspaceDocument(value)];
	if (value.kind !== "WorkspaceList") {
		throw new Error("Workspace import kind must be Workspace or WorkspaceList.");
	}
	const items = array(value.items, "WorkspaceList.items").map(parseWorkspaceDocument);
	const keys = new Set<string>();
	for (const item of items) {
		if (keys.has(item.sharedKey)) {
			throw new Error(`Duplicate workspace metadata.name: ${item.sharedKey}`);
		}
		keys.add(item.sharedKey);
	}
	return items;
}

export function cloneScope(scope: WorkspaceScope): WorkspaceScope {
	return parseScope(scope);
}

export function cloneShortcut(shortcut: WorkspaceShortcut): WorkspaceShortcut {
	return parseShortcut(shortcut);
}

function parseWorkspaceDocument(value: unknown): WorkspaceImportItem["workspace"] {
	const document = object(value, "Workspace");
	if (document.apiVersion !== WORKSPACE_EXPORT_API_VERSION || document.kind !== "Workspace") {
		throw new Error("Workspace item must use kubecove.dev/workspace/v1 Workspace.");
	}
	const metadata = object(document.metadata, "Workspace.metadata");
	const spec = object(document.spec, "Workspace.spec");
	const sharedKey = requiredString(metadata.name, "metadata.name");
	return {
		sharedKey,
		name: requiredString(spec.displayName, "spec.displayName"),
		scope: parseScope(spec.scope),
		shortcuts: array(spec.shortcuts ?? [], "spec.shortcuts").map(parseShortcut),
		portForwards: array(spec.portForwards ?? [], "spec.portForwards").map(parsePortForward),
	};
}

function parseScope(value: unknown): WorkspaceScope {
	const scope = object(value, "spec.scope");
	const clusterGroup =
		scope.clusterGroup === undefined ? undefined : object(scope.clusterGroup, "scope.clusterGroup");
	return {
		clusterContext: requiredString(scope.clusterContext, "scope.clusterContext"),
		clusterGroup: clusterGroup
			? {
					id: requiredString(clusterGroup.id, "scope.clusterGroup.id"),
					name: requiredString(clusterGroup.name, "scope.clusterGroup.name"),
					members: stringArray(clusterGroup.members, "scope.clusterGroup.members"),
				}
			: undefined,
		namespaces: stringArray(scope.namespaces, "scope.namespaces"),
		kinds: array(scope.kinds, "scope.kinds").map(parseKind),
		gitOpsFilter: optionalString(scope.gitOpsFilter),
		argoAppFilter: optionalString(scope.argoAppFilter) ?? "",
		layout: scope.layout === "resources" ? "resources" : "overview",
		shortcutPreferences: parseShortcutPreferences(scope.shortcutPreferences),
	};
}

function parseShortcut(value: unknown): WorkspaceShortcut {
	const shortcut = object(value, "shortcut");
	const kind = requiredString(shortcut.kind, "shortcut.kind");
	if (!["resources", "namespace", "argo", "compare"].includes(kind)) {
		throw new Error(`Unsupported shortcut kind: ${kind}`);
	}
	return {
		id: requiredString(shortcut.id, "shortcut.id"),
		label: requiredString(shortcut.label, "shortcut.label"),
		kind: kind as WorkspaceShortcut["kind"],
		namespace: optionalString(shortcut.namespace),
		argoApp: optionalString(shortcut.argoApp),
		compare: shortcut.compare === undefined ? undefined : parseCompare(shortcut.compare),
	};
}

function parsePortForward(value: unknown): SharedWorkspacePortForward {
	const forward = object(value, "portForward");
	return {
		clusterContext: requiredString(forward.clusterContext, "portForward.clusterContext"),
		namespace: requiredString(forward.namespace, "portForward.namespace"),
		serviceName: requiredString(forward.serviceName, "portForward.serviceName"),
		servicePort: portValue(forward.servicePort, "portForward.servicePort"),
		localPort: optionalPort(forward.localPort),
		label: optionalString(forward.label),
	};
}

function parseKind(value: unknown): ResourceKindSelection {
	if (typeof value === "string" && value.trim()) return value as ResourceKindSelection;
	const kind = object(value, "scope.kind") as unknown as DiscoveredResourceKind;
	if (typeof kind.namespaced !== "boolean") {
		throw new Error("kind.namespaced must be a boolean.");
	}
	return {
		group: requiredString(kind.group, "kind.group"),
		version: requiredString(kind.version, "kind.version"),
		apiVersion: requiredString(kind.apiVersion, "kind.apiVersion"),
		kind: requiredString(kind.kind, "kind.kind"),
		plural: requiredString(kind.plural, "kind.plural"),
		namespaced: kind.namespaced,
	};
}

function parseCompare(value: unknown): WorkspaceCompareEntry {
	const compare = object(value, "shortcut.compare");
	return {
		id: requiredString(compare.id, "compare.id"),
		kind: compare.kind === "namespaces" ? "namespaces" : "contexts",
		label: requiredString(compare.label, "compare.label"),
		leftLabel: requiredString(compare.leftLabel, "compare.leftLabel"),
		rightLabel: requiredString(compare.rightLabel, "compare.rightLabel"),
		clusterContexts: stringArray(compare.clusterContexts, "compare.clusterContexts"),
		namespaces: stringArray(compare.namespaces, "compare.namespaces"),
	};
}

function parseShortcutPreferences(value: unknown): WorkspaceShortcutPreferences | undefined {
	if (value === undefined) return undefined;
	const preferences = object(value, "scope.shortcutPreferences");
	return {
		includeResources: Boolean(preferences.includeResources),
		includeNamespaces: Boolean(preferences.includeNamespaces),
		includeCompare: Boolean(preferences.includeCompare),
		includeArgo: Boolean(preferences.includeArgo),
	};
}

function object(value: unknown, label: string): Record<string, unknown> {
	if (typeof value === "object" && value !== null && !Array.isArray(value)) {
		return value as Record<string, unknown>;
	}
	throw new Error(`${label} must be an object.`);
}

function array(value: unknown, label: string): unknown[] {
	if (Array.isArray(value)) return value;
	throw new Error(`${label} must be an array.`);
}

function stringArray(value: unknown, label: string): string[] {
	return array(value ?? [], label).map((item) => requiredString(item, label));
}

function requiredString(value: unknown, label: string): string {
	if (typeof value === "string" && value.trim()) return value.trim();
	throw new Error(`${label} must be a non-empty string.`);
}

function optionalString(value: unknown): string | undefined {
	return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function portValue(value: unknown, label: string): number {
	if (typeof value === "number" && Number.isInteger(value) && value >= 1 && value <= 65_535) {
		return value;
	}
	throw new Error(`${label} must be an integer from 1 to 65535.`);
}

function optionalPort(value: unknown): number | undefined {
	return value === undefined ? undefined : portValue(value, "portForward.localPort");
}
