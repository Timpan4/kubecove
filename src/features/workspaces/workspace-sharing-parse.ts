import type {
	WorkspaceCompareEntry,
	WorkspaceScope,
	WorkspaceShortcut,
	WorkspaceShortcutPreferences,
} from "@/lib/workspace-model";
import {
	CLUSTER_SCOPED_KINDS,
	type JsonObject,
	type JsonValue,
	type ResourceKindSelection,
	SUPPORTED_KINDS,
} from "@/lib/types";
import {
	WORKSPACE_EXPORT_API_VERSION,
	type SharedWorkspacePortForward,
	type WorkspaceImportItem,
} from "./workspace-sharing-schema";

export function parseWorkspaceImport(raw: string): WorkspaceImportItem["workspace"][] {
	let parsed: JsonValue;
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
	const serialized: JsonValue = JSON.parse(JSON.stringify(scope));
	return parseScope(serialized);
}

export function cloneShortcut(shortcut: WorkspaceShortcut): WorkspaceShortcut {
	const serialized: JsonValue = JSON.parse(JSON.stringify(shortcut));
	return parseShortcut(serialized);
}

function parseWorkspaceDocument(value: JsonValue | undefined): WorkspaceImportItem["workspace"] {
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

function parseScope(value: JsonValue | undefined): WorkspaceScope {
	const scope = object(value, "spec.scope");
	const clusterGroup =
		scope.clusterGroup === undefined ? undefined : object(scope.clusterGroup, "scope.clusterGroup");
	if (scope.layout !== "overview" && scope.layout !== "resources") {
		throw new Error("scope.layout must be overview or resources.");
	}
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
		layout: scope.layout,
		shortcutPreferences: parseShortcutPreferences(scope.shortcutPreferences),
	};
}

function parseShortcut(value: JsonValue | undefined): WorkspaceShortcut {
	const shortcut = object(value, "shortcut");
	const kind = requiredString(shortcut.kind, "shortcut.kind");
	if (kind !== "resources" && kind !== "namespace" && kind !== "argo" && kind !== "compare") {
		throw new Error(`Unsupported shortcut kind: ${kind}`);
	}
	return {
		id: requiredString(shortcut.id, "shortcut.id"),
		label: requiredString(shortcut.label, "shortcut.label"),
		kind,
		namespace: optionalString(shortcut.namespace),
		argoApp: optionalString(shortcut.argoApp),
		compare: shortcut.compare === undefined ? undefined : parseCompare(shortcut.compare),
	};
}

function parsePortForward(value: JsonValue | undefined): SharedWorkspacePortForward {
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

function parseKind(value: JsonValue | undefined): ResourceKindSelection {
	if (isString(value) && value.trim()) {
		const supportedKind = [...SUPPORTED_KINDS, ...CLUSTER_SCOPED_KINDS].find(
			(kind) => kind === value,
		);
		if (supportedKind) return supportedKind;
	}
	const kind = object(value, "scope.kind");
	if (!isBoolean(kind.namespaced)) {
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

function parseCompare(value: JsonValue | undefined): WorkspaceCompareEntry {
	const compare = object(value, "shortcut.compare");
	if (compare.kind !== "contexts" && compare.kind !== "namespaces") {
		throw new Error("compare.kind must be contexts or namespaces.");
	}
	return {
		id: requiredString(compare.id, "compare.id"),
		kind: compare.kind,
		label: requiredString(compare.label, "compare.label"),
		leftLabel: requiredString(compare.leftLabel, "compare.leftLabel"),
		rightLabel: requiredString(compare.rightLabel, "compare.rightLabel"),
		clusterContexts: stringArray(compare.clusterContexts, "compare.clusterContexts"),
		namespaces: stringArray(compare.namespaces, "compare.namespaces"),
	};
}

function parseShortcutPreferences(value: JsonValue | undefined): WorkspaceShortcutPreferences | undefined {
	if (value === undefined) return undefined;
	const preferences = object(value, "scope.shortcutPreferences");
	return {
		includeResources: booleanValue(
			preferences.includeResources,
			"scope.shortcutPreferences.includeResources",
		),
		includeNamespaces: booleanValue(
			preferences.includeNamespaces,
			"scope.shortcutPreferences.includeNamespaces",
		),
		includeCompare: booleanValue(
			preferences.includeCompare,
			"scope.shortcutPreferences.includeCompare",
		),
		includeArgo: booleanValue(preferences.includeArgo, "scope.shortcutPreferences.includeArgo"),
	};
}

function object(value: JsonValue | undefined, label: string): JsonObject {
	if (isRecord(value)) return value;
	throw new Error(`${label} must be an object.`);
}

function array(value: JsonValue | undefined, label: string): JsonValue[] {
	if (Array.isArray(value)) return value;
	throw new Error(`${label} must be an array.`);
}

function stringArray(value: JsonValue | undefined, label: string): string[] {
	return array(value ?? [], label).map((item) => requiredString(item, label));
}

function requiredString(value: JsonValue | undefined, label: string): string {
	if (isString(value) && value.trim()) return value.trim();
	throw new Error(`${label} must be a non-empty string.`);
}

function optionalString(value: JsonValue | undefined): string | undefined {
	return isString(value) && value.trim() ? value.trim() : undefined;
}

function booleanValue(value: JsonValue | undefined, label: string): boolean {
	if (isBoolean(value)) return value;
	throw new Error(`${label} must be a boolean.`);
}

function portValue(value: JsonValue | undefined, label: string): number {
	if (isNumber(value) && Number.isInteger(value) && value >= 1 && value <= 65_535) {
		return value;
	}
	throw new Error(`${label} must be an integer from 1 to 65535.`);
}

function optionalPort(value: JsonValue | undefined): number | undefined {
	return value === undefined ? undefined : portValue(value, "portForward.localPort");
}

function isRecord<Value>(value: Value): value is Value & JsonObject {
	return value !== null && !Array.isArray(value) && Object(value) === value;
}

function isString<Value>(value: Value): value is Value & string {
	return String(value) === value;
}

function isBoolean<Value>(value: Value): value is Value & boolean {
	return Boolean(value) === value;
}

function isNumber<Value>(value: Value): value is Value & number {
	return Number(value) === value;
}
