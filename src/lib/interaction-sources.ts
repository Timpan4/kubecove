// Names the code that owns a DOM node for the interaction recorder: the Svelte
// source location in dev builds, otherwise the nearest test id or role. Never
// text content or cluster data.

type ElementWithMeta = Element & {
	__svelte_meta?: { loc?: { file: string; line: number } } | null;
};

const UNATTRIBUTED = "(unattributed)";
let sourceCache = new WeakMap<Node, string>();

function ownSource(node: Node): string | undefined {
	// SAFETY: Svelte dev builds set the optional __svelte_meta field on nodes they create; other nodes lack it.
	const loc = (node as ElementWithMeta).__svelte_meta?.loc;
	return loc ? `${loc.file}:${loc.line}` : undefined;
}

function fallbackSource(node: Node): string | undefined {
	if (!(node instanceof Element)) return undefined;
	const testId = node.getAttribute("data-testid");
	if (testId) return `[data-testid=${testId}]`;
	const role = node.getAttribute("role");
	return role ? `[role=${role}]` : undefined;
}

// Nearest Svelte source location; release builds have none, so use the nearest
// test id or role instead.
export function sourceOf(node: Node | null): string {
	const visited: Node[] = [];
	let fallback: string | undefined;
	let result: string | undefined;
	for (let cursor = node; cursor; cursor = cursor.parentNode) {
		const cached = sourceCache.get(cursor);
		if (cached) {
			result = cached;
			break;
		}
		visited.push(cursor);
		result = ownSource(cursor);
		if (result) break;
		fallback ??= fallbackSource(cursor);
	}
	result ??= fallback ?? UNATTRIBUTED;
	for (const seen of visited) sourceCache.set(seen, result);
	return result;
}

export function describeTarget(target: EventTarget | null): string {
	if (!(target instanceof Element)) return "(window)";
	return `${target.tagName.toLowerCase()} ${sourceOf(target)}`;
}

// Attribute rebuilt subtrees to the template that created or destroyed them.
export function subtreeSource(record: MutationRecord): string {
	for (const node of [record.addedNodes[0], record.removedNodes[0]]) {
		const source = node ? sourceOf(node) : UNATTRIBUTED;
		if (source !== UNATTRIBUTED) return source;
	}
	return sourceOf(record.target);
}

export function resetSourceCache(): void {
	sourceCache = new WeakMap();
}
