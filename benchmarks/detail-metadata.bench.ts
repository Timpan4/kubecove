import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	buildCuratedMetadata,
	formatObjectRows,
	visibleMetadataBadges,
} from "@/features/resource-detail/metadata-details";
import type { JsonObject, ResourceSummary } from "@/lib/types";

const metadata: JsonObject = {
	name: "checkout-api-7d8d9f4b6d-x9k2p",
	namespace: "checkout",
	uid: "synthetic-resource-uid",
	resourceVersion: "882001",
	generation: 14,
	creationTimestamp: "2026-01-01T00:00:00.000Z",
	ownerReferences: Array.from({ length: 25 }, (_, index) => ({
		apiVersion: "apps/v1",
		kind: index === 0 ? "ReplicaSet" : "Job",
		name: `owner-${index}`,
		uid: `owner-uid-${index}`,
		controller: index === 0,
	})),
	finalizers: ["kubernetes.io/pvc-protection", "operator.example.com/cleanup"],
	labels: Object.fromEntries(
		Array.from({ length: 250 }, (_, index) => [
			`label-${index}`,
			`value-${index % 20}`,
		]),
	),
	annotations: Object.fromEntries(
		Array.from({ length: 250 }, (_, index) => [
			`annotation-${index}`,
			index % 10 === 0
				? `line ${index}\nlarge value ${index}`
				: `value-${index}`,
		]),
	),
	managedFields: Array.from({ length: 100 }, (_, index) => ({
		manager: `controller-${index % 12}`,
		operation: index % 2 === 0 ? "Apply" : "Update",
		apiVersion: "apps/v1",
		time: "2026-01-01T00:00:00Z",
		fieldsType: "FieldsV1",
		fieldsV1: {
			f: { metadata: { f: { labels: { [`f:label-${index}`]: {} } } } },
		},
	})),
};
const status: JsonObject = {
	conditions: Array.from({ length: 20 }, (_, index) => ({
		type: `Condition${index}`,
		status: index % 2 === 0 ? "True" : "False",
		message: `condition detail ${index}`,
	})),
	containerStatuses: Array.from({ length: 40 }, (_, index) => ({
		name: `container-${index}`,
		ready: index % 5 !== 0,
		restartCount: index % 3,
		state: { running: { startedAt: "2026-01-01T00:00:00Z" } },
	})),
};
const resource: ResourceSummary = {
	cluster: "prod",
	apiVersion: "apps/v1",
	kind: "Deployment",
	name: "checkout-api",
	namespace: "checkout",
	age: "1d",
	health: "healthy",
	ownerRef: "checkout-api-replicaset",
	argoApp: "checkout",
	helmRelease: "checkout-api",
};
const curated = buildCuratedMetadata(metadata, resource);
const rawMetadataRows = formatObjectRows(metadata);
const rawStatusRows = formatObjectRows(status);
const visibleLabels = visibleMetadataBadges(curated.labels, false);
const visibleAnnotations = visibleMetadataBadges(curated.annotations, false);

assert.equal(curated.labels.length, 250);
assert.equal(curated.annotations.length, 225);
assert.equal(visibleLabels.badges.length, 4);
assert.equal(visibleAnnotations.hiddenCount, 221);
assert.equal(rawMetadataRows.length, Object.keys(metadata).length);
assert.equal(rawStatusRows.length, Object.keys(status).length);

describe("F12 curated and raw resource detail preparation", () => {
	bench(
		"F12 curated ownership, labels, annotations, and manager summary",
		() => {
			const next = buildCuratedMetadata(metadata, resource);
			visibleMetadataBadges(next.labels, false);
			visibleMetadataBadges(next.annotations, false);
		},
	);

	bench("F12 raw metadata and status row formatting", () => {
		formatObjectRows(metadata);
		formatObjectRows(status);
	});
});
