import type { ResourceSummary } from "../../lib/types";

export interface CuratedMetadataField {
	label: string;
	value: string;
}

export interface CuratedMetadataBadge {
	key: string;
	value: string;
}

export interface CuratedMetadataModel {
	identity: CuratedMetadataField[];
	lifecycle: CuratedMetadataField[];
	naming: CuratedMetadataField[];
	ownership: CuratedMetadataField[];
	labels: CuratedMetadataBadge[];
	annotations: CuratedMetadataBadge[];
	annotationCount: number;
	management: CuratedMetadataField[];
}

export function visibleMetadataBadges(
	badges: CuratedMetadataBadge[],
	expanded: boolean,
	limit = 4,
): { badges: CuratedMetadataBadge[]; hiddenCount: number } {
	if (expanded || badges.length <= limit) return { badges, hiddenCount: 0 };
	return {
		badges: badges.slice(0, limit),
		hiddenCount: badges.length - limit,
	};
}

export function buildCuratedMetadata(
	metadata: Record<string, unknown>,
	resource: ResourceSummary,
): CuratedMetadataModel {
	const owner = ownerReferenceFields(metadata.ownerReferences);
	const labels = metadataBadgeEntries(metadata.labels);
	const annotations = metadataBadgeEntries(metadata.annotations).filter(({ value }) =>
		isInlineAnnotationValue(value),
	);
	const managerSummary = managedFieldsSummary(metadata.managedFields);

	return {
		identity: compactFields([
			field("Name", stringValue(metadata.name) ?? resource.name),
			field(
				"Namespace",
				stringValue(metadata.namespace) ??
					resource.namespace ??
					"cluster-scoped",
			),
			field("UID", stringValue(metadata.uid)),
			field("Resource Version", stringValue(metadata.resourceVersion)),
		]),
		lifecycle: compactFields([
			field(
				"Created",
				stringValue(metadata.creationTimestamp) ?? resource.createdAt,
			),
			field("Age", resource.age),
			field("Generation", primitiveValue(metadata.generation)),
			field(
				"Deletion",
				stringValue(metadata.deletionTimestamp) ?? "not scheduled",
			),
			field("Finalizers", finalizersLabel(metadata.finalizers)),
		]),
		naming: compactFields([
			field("Generate name", stringValue(metadata.generateName)),
			field("Kind", resource.kind),
			field("API Version", resource.apiVersion),
		]),
		ownership: compactFields([
			...owner,
			field("Owner", owner.length === 0 ? resource.ownerRef : undefined),
			field("Argo", resource.argoApp),
			field("Helm", resource.helmRelease),
			field("GitOps", resource.gitOpsOwner?.name),
		]),
		labels,
		annotations,
		annotationCount: metadataBadgeEntries(metadata.annotations).length,
		management: compactFields([
			field("Managers", managerSummary),
			field(
				"Raw managedFields",
				managerSummary ? "advanced metadata" : undefined,
			),
		]),
	};
}

function field(label: string, value: string | undefined): CuratedMetadataField | null {
	if (!value) return null;
	return { label, value };
}

function compactFields(
	fields: Array<CuratedMetadataField | null>,
): CuratedMetadataField[] {
	return fields.filter((item): item is CuratedMetadataField => Boolean(item));
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringValue(value: unknown): string | undefined {
	return typeof value === "string" && value.length > 0 ? value : undefined;
}

function primitiveValue(value: unknown): string | undefined {
	if (typeof value === "string") return value || undefined;
	if (typeof value === "number" || typeof value === "boolean") return String(value);
	return undefined;
}

function metadataBadgeEntries(value: unknown): CuratedMetadataBadge[] {
	if (!isRecord(value)) return [];
	return Object.entries(value)
		.map(([key, item]) => ({ key, value: primitiveValue(item) ?? "" }))
		.filter((item) => item.value.length > 0)
		.sort((left, right) => left.key.localeCompare(right.key));
}

function isInlineAnnotationValue(value: string): boolean {
	const trimmed = value.trim();
	return (
		trimmed.length > 0 &&
		trimmed.length <= 96 &&
		!trimmed.includes("\n") &&
		!trimmed.startsWith("{") &&
		!trimmed.startsWith("[")
	);
}

function finalizersLabel(value: unknown): string | undefined {
	if (!Array.isArray(value)) return "none";
	const finalizers = value.filter((item): item is string => typeof item === "string");
	return finalizers.length === 0 ? "none" : finalizers.join(", ");
}

function ownerReferenceFields(value: unknown): CuratedMetadataField[] {
	if (!Array.isArray(value)) return [];
	const owners = value.filter(isRecord);
	const owner =
		owners.find((item) => item.controller === true) ?? owners[0];
	if (!owner) return [];
	return compactFields([
		field("Controller", stringValue(owner.kind)),
		field("Owner", stringValue(owner.name)),
		field("Owner UID", stringValue(owner.uid)),
	]);
}

function managedFieldsSummary(value: unknown): string | undefined {
	if (!Array.isArray(value)) return undefined;
	const managers = [
		...new Set(
			value
				.filter(isRecord)
				.map((item) => stringValue(item.manager))
				.filter((manager): manager is string => Boolean(manager)),
		),
	];
	if (managers.length === 0) return undefined;
	const visible = managers.slice(0, 4).join(", ");
	return managers.length > 4 ? `${visible} +${managers.length - 4} more` : visible;
}
