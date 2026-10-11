import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import { serializePersistedWorkspaces } from "@/features/workspaces/workspaceStore";
import type {
	ResourceEntryPointCoverage,
	WorkspaceEntryPoints,
} from "@/lib/workspace-entry-points";
import {
	entryPointFromNamespace,
	entryPointFromResource,
	entryPointKey,
	entryPointsEqual,
	isPinnedEntry,
	normalizeEntryPoints,
	reconcileEntryPoints,
	recordRecentEntry,
	updateWorkspaceEntryPointsInCollection,
} from "@/lib/workspace-entry-points";
import {
	buildWorkspaceCompareEntries,
	buildWorkspaceCompareSummaries,
	buildWorkspaceHealthSummary,
} from "@/lib/workspace-model";
import {
	savedWorkspace,
	workspaceRows,
	workspaceScope,
} from "./workspace-benchmark-fixtures";

const scope = workspaceScope();
const compareEntries = buildWorkspaceCompareEntries(scope);
const healthSummary = buildWorkspaceHealthSummary(workspaceRows);
const compareSummaries = buildWorkspaceCompareSummaries(
	compareEntries,
	workspaceRows,
);
const pinned = workspaceRows
	.filter((_, index) => index % 20 === 0)
	.map((row, index) =>
		entryPointFromResource(
			row,
			`2026-03-${String((index % 28) + 1).padStart(2, "0")}T00:00:00Z`,
		),
	);
const firstPinned = pinned[0];
if (!firstPinned) throw new Error("Missing pinned identity fixture");
const duplicatePinEntries: WorkspaceEntryPoints = {
	pinned: [
		firstPinned,
		{ ...firstPinned, lastVisitedAt: "2026-04-02T00:00:00Z" },
		{ ...firstPinned, clusterContext: "prod/context-1" },
		{ ...firstPinned, namespace: "other-namespace" },
		{ ...firstPinned, apiVersion: "apps/v1beta1" },
		{ ...firstPinned, resourceKind: "StatefulSet" },
	],
	recent: [],
};
const normalizedDuplicatePins = normalizeEntryPoints(duplicatePinEntries);
const recent = pinned.slice(0, 8);
const entryPoints: WorkspaceEntryPoints = { pinned, recent };
const requestedKinds = [
	"Pod",
	"Deployment",
	"ReplicaSet",
	"Service",
	"ConfigMap",
	"Secret",
] as const;
const coverage: ResourceEntryPointCoverage[] = Array.from(
	{ length: 3 },
	(_, contextIndex) => ({
		clusterContext: `prod/context-${contextIndex}`,
		requests: Array.from({ length: 200 }, (_, requestIndex) => ({
			kind: requestedKinds[requestIndex % requestedKinds.length] ?? "Pod",
			namespace: `namespace-${Math.floor(requestIndex / 2) % 100}`,
		})),
	}),
);
const reconciled = reconcileEntryPoints(entryPoints, workspaceRows, coverage);
const normalized = normalizeEntryPoints(entryPoints);
const workspaceCollection = Array.from({ length: 100 }, (_, index) =>
	savedWorkspace(index),
);
const targetWorkspace = workspaceCollection[42];
const comparedEntry = compareEntries[0];
const reviewedEntry = pinned[42];
if (!targetWorkspace || !comparedEntry || !reviewedEntry)
	throw new Error("Missing workspace fixture entries");
const changedResource = entryPointFromResource(
	{
		cluster: "prod/context-2",
		apiVersion: "apps/v1",
		kind: "Deployment",
		name: "newly-observed-resource",
		namespace: "namespace-99",
		age: "1m",
		health: "unknown",
	},
	"2026-04-01T00:00:00Z",
);
const changedCollection = updateWorkspaceEntryPointsInCollection(
	workspaceCollection,
	targetWorkspace.id,
	(current, now) =>
		recordRecentEntry(current, { ...changedResource, lastVisitedAt: now }),
	"2026-04-01T00:00:00Z",
);
const unchangedCollection = updateWorkspaceEntryPointsInCollection(
	workspaceCollection,
	targetWorkspace.id,
	(current) => current,
	"2026-04-01T00:00:00Z",
);
const persistedPayload = changedCollection
	? serializePersistedWorkspaces(changedCollection)
	: "";
const presentResource = {
	...workspaceRows[0],
	cluster: "prod/context-0",
	kind: "Pod",
	name: "coverage-present",
	apiVersion: "v1",
	namespace: "namespace-0",
};
const absentCoveredResource = {
	...presentResource,
	name: "coverage-absent",
};
const uncoveredResource = {
	...presentResource,
	kind: "Deployment",
	name: "coverage-uncovered",
	apiVersion: "apps/v1",
};
const presentEntry = entryPointFromResource(
	presentResource,
	"2026-04-03T00:00:00Z",
);
const absentCoveredEntry = entryPointFromResource(
	absentCoveredResource,
	"2026-04-04T00:00:00Z",
);
const uncoveredEntry = entryPointFromResource(
	uncoveredResource,
	"2026-04-05T00:00:00Z",
);
const namespaceEntry = entryPointFromNamespace(
	"prod/context-0",
	"namespace-0",
	"2026-04-06T00:00:00Z",
);
const reconciliationFixture: WorkspaceEntryPoints = {
	pinned: [
		presentEntry,
		absentCoveredEntry,
		uncoveredEntry,
		{ ...presentEntry, lastVisitedAt: "2026-04-07T00:00:00Z" },
		namespaceEntry,
	],
	recent: [
		absentCoveredEntry,
		presentEntry,
		uncoveredEntry,
		{ ...absentCoveredEntry, lastVisitedAt: "2026-04-08T00:00:00Z" },
	],
};
const reconciliationCoverage: ResourceEntryPointCoverage[] = [
	{
		clusterContext: "prod/context-0",
		requests: [{ kind: "Pod", namespace: "namespace-0" }],
	},
];
const reconciledFixture = reconcileEntryPoints(
	reconciliationFixture,
	[presentResource],
	reconciliationCoverage,
);

assert.equal(workspaceRows.length, 10_000);
assert.equal(healthSummary.total, workspaceRows.length);
assert(compareSummaries.length <= 2 && compareSummaries.length === 2);
assert.equal(
	compareSummaries[0]?.left.total + compareSummaries[0]?.right.total,
	workspaceRows.filter(
		(row) =>
			row.cluster === comparedEntry.leftLabel ||
			row.cluster === comparedEntry.rightLabel,
	).length,
);
assert.equal(normalized.pinned.length, pinned.length);
assert.equal(normalizedDuplicatePins.pinned.length, 5);
assert.equal(normalizedDuplicatePins.pinned[0]?.lastVisitedAt, firstPinned.lastVisitedAt);
assert.deepEqual(
	normalizedDuplicatePins.pinned.map(entryPointKey),
	[
		entryPointKey(firstPinned),
		entryPointKey({ ...firstPinned, clusterContext: "prod/context-1" }),
		entryPointKey({ ...firstPinned, namespace: "other-namespace" }),
		entryPointKey({ ...firstPinned, apiVersion: "apps/v1beta1" }),
		entryPointKey({ ...firstPinned, resourceKind: "StatefulSet" }),
	],
);
assert(isPinnedEntry(entryPoints, reviewedEntry));
assert(entryPointKey(reviewedEntry).length > 0);
assert(reconciled.pinned.length <= normalized.pinned.length);
assert(unchangedCollection === null);
assert(changedCollection && changedCollection !== workspaceCollection);
assert(persistedPayload.length > 0);
assert.equal(
	entryPointsEqual(normalized, normalizeEntryPoints(normalized)),
	true,
);
assert.deepEqual(
	reconciledFixture.pinned.map(entryPointKey),
	[entryPointKey(presentEntry), entryPointKey(uncoveredEntry), entryPointKey(namespaceEntry)],
);
assert.deepEqual(
	reconciledFixture.recent.map(entryPointKey),
	[entryPointKey(presentEntry), entryPointKey(uncoveredEntry)],
);

describe("F31 workspace health and selected comparisons; F32 entry reconciliation with folded F30 and F35", () => {
	bench(
		"F31 full health summary and two selected partition comparisons",
		() => {
			buildWorkspaceHealthSummary(workspaceRows);
			buildWorkspaceCompareSummaries(compareEntries, workspaceRows);
		},
	);

	bench("F32 live resource reconciliation and retained pin membership", () => {
		const next = reconcileEntryPoints(entryPoints, workspaceRows, coverage);
		for (const entry of next.pinned) isPinnedEntry(next, entry);
		normalizeEntryPoints(duplicatePinEntries);
		reconcileEntryPoints(
			reconciliationFixture,
			[presentResource],
			reconciliationCoverage,
		);
	});

	bench("F32 unchanged update normalization and equality short-circuit", () => {
		const next = updateWorkspaceEntryPointsInCollection(
			workspaceCollection,
			targetWorkspace.id,
			(current) => current,
			"2026-04-01T00:00:00Z",
		);
		if (next)
			entryPointsEqual(next[42]?.entryPoints, targetWorkspace.entryPoints);
	});

	bench(
		"F35 changed entry update and full persisted workspace serialization",
		() => {
			const next = updateWorkspaceEntryPointsInCollection(
				workspaceCollection,
				targetWorkspace.id,
				(current, now) =>
					recordRecentEntry(current, {
						...changedResource,
						lastVisitedAt: now,
					}),
				"2026-04-01T00:00:00Z",
			);
			if (next) serializePersistedWorkspaces(next);
		},
	);
});
