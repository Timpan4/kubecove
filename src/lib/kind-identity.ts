import { watchKeySignature } from "./resource-watch";
import {
	type AnyKind,
	CLUSTER_SCOPED_KINDS,
	type DiscoveredResourceKind,
	type ResourceKindSelection,
	type ResourceListRequest,
	type ResourceSummary,
	SUPPORTED_KINDS,
	type WatchResourceKey,
	type WatchResourceKind,
} from "./types";

// Kind identity: the one key, label, scope, and resource match for a browsed
// kind. Built-in kinds are identified by name, Custom Resources kinds by
// apiVersion, plural, and kind. Persisted kind selections store the kind
// itself, not a key. Sidebar tree node ids store the key too; for Custom
// Resources it is the same text as before this module, so saved nodes still match.

const BUILTIN_KINDS: readonly string[] = [...SUPPORTED_KINDS, ...CLUSTER_SCOPED_KINDS];
const CLUSTER_SCOPED_BUILTIN_KINDS: readonly string[] = CLUSTER_SCOPED_KINDS;

export function isBuiltinKind(name: string): name is AnyKind {
	return BUILTIN_KINDS.includes(name);
}

export function isCustomResourceKind(kind: string | DiscoveredResourceKind): kind is DiscoveredResourceKind {
	return Object(kind) === kind;
}

/** A built-in kind's name, or `apiVersion/plural/kind` for a Custom Resources kind; the two never collide. */
export function kindKey(kind: ResourceKindSelection): string {
	return isCustomResourceKind(kind) ? `${kind.apiVersion}/${kind.plural}/${kind.kind}` : kind;
}

export function kindLabel(kind: ResourceKindSelection): string {
	return isCustomResourceKind(kind) ? kind.kind : kind;
}

export function isClusterScopedKind(kind: string | DiscoveredResourceKind): boolean {
	return isCustomResourceKind(kind) ? !kind.namespaced : CLUSTER_SCOPED_BUILTIN_KINDS.includes(kind);
}

/** The kinds in order, each kept once. */
export function uniqueKinds(kinds: ResourceKindSelection[]): ResourceKindSelection[] {
	const seen = new Set<string>();
	const unique: ResourceKindSelection[] = [];
	for (const kind of kinds) {
		const key = kindKey(kind);
		if (seen.has(key)) continue;
		seen.add(key);
		unique.push(kind);
	}
	return unique;
}

/** True when both hold the same kinds, in any order. */
export function sameKinds(left: ResourceKindSelection[], right: ResourceKindSelection[]): boolean {
	const keys = new Set(left.map(kindKey));
	const rightKeys = new Set(right.map(kindKey));
	return keys.size === rightKeys.size && [...rightKeys].every((key) => keys.has(key));
}

/**
 * Whether a resource is of one of the kinds. A Custom Resources kind needs the
 * resource's apiVersion; as with Resource identity, a resource without one,
 * or a built-in kind, which names none, matches on the kind name.
 */
export function kindMatcher(
	kinds: ResourceKindSelection[],
): (resource: Pick<ResourceSummary, "kind" | "apiVersion">) => boolean {
	const builtinNames = new Set<string>();
	const customNames = new Set<string>();
	const customVersions = new Set<string>();
	for (const kind of kinds) {
		if (!isCustomResourceKind(kind)) {
			builtinNames.add(kind);
			continue;
		}
		customNames.add(kind.kind);
		customVersions.add(`${kind.apiVersion}/${kind.kind}`);
	}
	return ({ kind, apiVersion }) =>
		builtinNames.has(kind) ||
		(apiVersion ? customVersions.has(`${apiVersion}/${kind}`) : customNames.has(kind));
}

/** One list read: a kind in one namespace, or in every namespace when `namespace` is absent. */
export interface KindFetchKey {
	kind: ResourceKindSelection;
	namespace?: string;
}

/** Cluster-scoped kinds, and every kind when no namespace is selected, read once; namespaced kinds read per namespace. */
export function kindFetchKeys(namespaces: string[], kinds: ResourceKindSelection[]): KindFetchKey[] {
	const keys: KindFetchKey[] = [];
	for (const kind of kinds) {
		if (namespaces.length === 0 || isClusterScopedKind(kind)) {
			keys.push({ kind });
			continue;
		}
		for (const namespace of namespaces) keys.push({ kind, namespace });
	}
	return keys;
}

export function kindListRequest({ kind, namespace }: KindFetchKey): ResourceListRequest {
	const scope = namespace || undefined;
	return isCustomResourceKind(kind) ? { resourceKind: kind, namespace: scope } : { kind, namespace: scope };
}

/** The watch kind for a kind selection or a listed resource's kind name. */
function watchKind(kind: string | DiscoveredResourceKind): WatchResourceKind {
	if (!isCustomResourceKind(kind)) return { kind };
	const { group, version, apiVersion, plural, namespaced } = kind;
	return { kind: kind.kind, group, version, apiVersion, plural, namespaced };
}

/** The watch keys, one per kind and namespace; a later duplicate replaces an earlier one. */
export function mergeWatchKeys(...groups: WatchResourceKey[][]): WatchResourceKey[] {
	const merged = new Map<string, WatchResourceKey>();
	for (const group of groups) {
		for (const key of group) merged.set(watchKeySignature(key), key);
	}
	return Array.from(merged.values());
}

export function kindWatchKeys(keys: KindFetchKey[]): WatchResourceKey[] {
	return mergeWatchKeys(keys.map(({ kind, namespace }) => ({ resourceKind: watchKind(kind), namespace })));
}
