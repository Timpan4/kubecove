import type {
	ResourceActionEdits,
	ResourceActionState,
	ResourceContainerEdit,
} from "@/lib/types";
import type { ResourceActionRowId } from "./resource-actions-model";

// ADR 0024 drafts are deltas against the loaded action state: a removal sends
// `remove: true` or `null` only for what the baseline has, and drops a draft
// that never reached it.

export type QuantityField = "requests" | "limits";
export type MetadataField = "labels" | "annotations";

/** One change to the pending drafts, decided against the loaded baseline. Returns `edits` unchanged to refuse. */
export type DraftChange = (edits: ResourceActionEdits, base: ResourceActionState) => ResourceActionEdits;

function emptyContainerEdit(name: string): ResourceContainerEdit {
	return { name, env: [], requests: {}, limits: {} };
}

export function containerDraft(edits: ResourceActionEdits, name: string): ResourceContainerEdit {
	return edits.containers?.find((container) => container.name === name) ?? emptyContainerEdit(name);
}

function updateContainer(
	edits: ResourceActionEdits,
	name: string,
	update: (current: ResourceContainerEdit) => ResourceContainerEdit,
): ResourceActionEdits {
	const others = (edits.containers ?? []).filter((container) => container.name !== name);
	return { ...edits, containers: [...others, update(containerDraft(edits, name))] };
}

function baseContainer(base: ResourceActionState, name: string) {
	return base.containers.find((container) => container.name === name);
}

export const clearDrafts: DraftChange = () => ({});

export const setReplicas = (value: number): DraftChange => (edits) =>
	Number.isSafeInteger(value) && value >= 0 ? { ...edits, replicas: value } : edits;

export const setImage = (container: string, image: string): DraftChange => (edits) =>
	updateContainer(edits, container, (current) => ({ ...current, image }));

export const setEnvironment = (container: string, variable: string, value: string): DraftChange => (edits) =>
	updateContainer(edits, container, (current) => ({
		...current,
		env: [...(current.env ?? []).filter((item) => item.name !== variable), { name: variable, value }],
	}));

/** Refuses a blank name or one the baseline container already defines. */
export const addEnvironment = (container: string, variable: string, value: string): DraftChange => (edits, base) => {
	const name = variable.trim();
	if (!name || baseContainer(base, container)?.env.some((item) => item.name === name)) return edits;
	return setEnvironment(container, name, value)(edits, base);
};

export const renameEnvironment = (container: string, previousName: string, nextName: string): DraftChange => (edits) =>
	updateContainer(edits, container, (current) => ({
		...current,
		env: (current.env ?? []).map((item) => (item.name === previousName ? { ...item, name: nextName } : item)),
	}));

export const removeEnvironment = (container: string, variable: string): DraftChange => (edits, base) => {
	const inBase = baseContainer(base, container)?.env.some((item) => item.name === variable);
	return updateContainer(edits, container, (current) => {
		const others = (current.env ?? []).filter((item) => item.name !== variable);
		return { ...current, env: inBase ? [...others, { name: variable, remove: true }] : others };
	});
};

export const restoreEnvironment = (container: string, variable: string): DraftChange => (edits) =>
	updateContainer(edits, container, (current) => ({
		...current,
		env: (current.env ?? []).filter((item) => item.name !== variable),
	}));

function withQuantity(
	edits: ResourceActionEdits,
	container: string,
	field: QuantityField,
	key: string,
	value: string | null | undefined,
): ResourceActionEdits {
	return updateContainer(edits, container, (current) => {
		const map = { ...current[field] };
		if (value === undefined) delete map[key];
		else map[key] = value;
		return { ...current, [field]: map };
	});
}

export const setQuantity = (container: string, field: QuantityField, key: string, value: string): DraftChange => (edits) =>
	withQuantity(edits, container, field, key, value);

export const addQuantity = (container: string, field: QuantityField, key: string, value: string): DraftChange => (edits) => {
	const name = key.trim();
	return name ? withQuantity(edits, container, field, name, value) : edits;
};

export const removeQuantity = (container: string, field: QuantityField, key: string): DraftChange => (edits, base) => {
	const inBase = Object.hasOwn(baseContainer(base, container)?.[field] ?? {}, key);
	return withQuantity(edits, container, field, key, inBase ? null : undefined);
};

export const restoreQuantity = (container: string, field: QuantityField, key: string): DraftChange => (edits) =>
	withQuantity(edits, container, field, key, undefined);

export const setMetadata = (field: MetadataField, key: string, value: string): DraftChange => (edits) => ({
	...edits,
	[field]: { ...edits[field], [key]: value },
});

export const addMetadata = (field: MetadataField, key: string, value: string): DraftChange => (edits, base) => {
	const name = key.trim();
	return name ? setMetadata(field, name, value)(edits, base) : edits;
};

export const removeMetadata = (field: MetadataField, key: string): DraftChange => (edits, base) => {
	const changes = { ...edits[field] };
	if (Object.hasOwn(base[field], key)) changes[key] = null;
	else delete changes[key];
	return { ...edits, [field]: changes };
};

function hasContainerChanges(container: ResourceContainerEdit): boolean {
	return (
		container.image !== undefined ||
		Boolean(container.env?.length) ||
		Object.keys(container.requests ?? {}).length > 0 ||
		Object.keys(container.limits ?? {}).length > 0
	);
}

/** Drops the drafts one action row collected. */
export const revertRow = (row: ResourceActionRowId): DraftChange => (edits) => {
	if (row === "replicas") {
		const { replicas: _replicas, ...rest } = edits;
		return rest;
	}
	if (row === "labels" || row === "annotations") {
		const { [row]: _metadata, ...rest } = edits;
		return rest;
	}
	if (row !== "image" && row !== "environment" && row !== "resources") return edits;
	const containers = (edits.containers ?? [])
		.map((container): ResourceContainerEdit => {
			if (row === "image") {
				const { image: _image, ...rest } = container;
				return rest;
			}
			if (row === "environment") return { ...container, env: [] };
			return { ...container, requests: {}, limits: {} };
		})
		.filter(hasContainerChanges);
	return { ...edits, containers };
};

export function pendingEditCount(edits: ResourceActionEdits): number {
	return (
		(edits.replicas === undefined ? 0 : 1) +
		(edits.containers ?? []).reduce(
			(total, container) =>
				total +
				(container.image === undefined ? 0 : 1) +
				(container.env?.length ?? 0) +
				Object.keys(container.requests ?? {}).length +
				Object.keys(container.limits ?? {}).length,
			0,
		) +
		Object.keys(edits.labels ?? {}).length +
		Object.keys(edits.annotations ?? {}).length
	);
}

export function hasPendingEdits(edits: ResourceActionEdits): boolean {
	return pendingEditCount(edits) > 0;
}

function plural(count: number, noun: string): string {
	return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

/** The row's pending drafts in words, or "" when it has none. */
export function pendingRowLabel(edits: ResourceActionEdits, row: ResourceActionRowId): string {
	const containers = edits.containers ?? [];
	if (row === "replicas") return edits.replicas === undefined ? "" : `Pending: ${edits.replicas}`;
	if (row === "image") {
		const changed = containers.filter((container) => container.image !== undefined);
		return changed.length ? `Pending: ${changed.map((item) => `${item.name} ${item.image}`).join(", ")}` : "";
	}
	if (row === "environment") {
		const count = containers.reduce((total, container) => total + (container.env?.length ?? 0), 0);
		return count ? `Pending: ${plural(count, "environment change")}` : "";
	}
	if (row === "resources") {
		const count = containers.reduce(
			(total, container) =>
				total + Object.keys(container.requests ?? {}).length + Object.keys(container.limits ?? {}).length,
			0,
		);
		return count ? `Pending: ${plural(count, "resource change")}` : "";
	}
	if (row === "labels" || row === "annotations") {
		const count = Object.keys(edits[row] ?? {}).length;
		return count ? `Pending: ${plural(count, row === "labels" ? "label change" : "annotation change")}` : "";
	}
	return "";
}

export interface EnvironmentRow {
	name: string;
	value: string;
	referenced: boolean;
	removed: boolean;
	isNew: boolean;
}

/** Baseline variables with their drafts applied, then new variables. */
export function environmentRows(
	edits: ResourceActionEdits,
	base: ResourceActionState,
	containerName: string,
): EnvironmentRow[] {
	const container = baseContainer(base, containerName);
	if (!container) return [];
	const changes = containerDraft(edits, containerName).env ?? [];
	const rows: EnvironmentRow[] = container.env.map((variable) => {
		const change = changes.find((item) => item.name === variable.name);
		return {
			name: variable.name,
			value: change?.remove ? (variable.value ?? "") : (change?.value ?? variable.value ?? ""),
			referenced: variable.referenced,
			removed: Boolean(change?.remove),
			isNew: false,
		};
	});
	for (const change of changes) {
		if (change.remove || container.env.some((variable) => variable.name === change.name)) continue;
		rows.push({ name: change.name, value: change.value ?? "", referenced: false, removed: false, isNew: true });
	}
	return rows;
}

export interface QuantityRow {
	key: string;
	value: string;
	removed: boolean;
}

export function quantityRows(
	edits: ResourceActionEdits,
	base: ResourceActionState,
	containerName: string,
	field: QuantityField,
): QuantityRow[] {
	const baseline = baseContainer(base, containerName)?.[field] ?? {};
	const changes = containerDraft(edits, containerName)[field] ?? {};
	return [...new Set([...Object.keys(baseline), ...Object.keys(changes)])].map((key) => ({
		key,
		value: Object.hasOwn(changes, key) && changes[key] !== null ? (changes[key] ?? "") : (baseline[key] ?? ""),
		removed: changes[key] === null,
	}));
}

/** Entries that remain after the drafts, with drafted values. */
export function metadataRows(
	edits: ResourceActionEdits,
	base: ResourceActionState,
	field: MetadataField,
): Array<{ key: string; value: string }> {
	const changes = edits[field] ?? {};
	return [...new Set([...Object.keys(base[field]), ...Object.keys(changes)])]
		.filter((key) => changes[key] !== null)
		.map((key) => ({ key, value: changes[key] ?? base[field][key] ?? "" }));
}
