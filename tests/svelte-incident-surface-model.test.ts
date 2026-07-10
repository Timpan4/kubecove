import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	buildIncidentFilterOptions,
	incidentCaseSummary,
	incidentCaseTitle,
	incidentDetailPivots,
	incidentItemKey,
	incidentKnownSummary,
	incidentMissingSummary,
	incidentNextSummary,
	incidentResourcesHealthFilter,
	incidentScopeLabel,
	incidentSeverityLabel,
	incidentSignalSummary,
	incidentWarningSummary,
	isIncidentResourceSelected,
} from "../src/app/svelte/incidentSurfaceModel";
import {
	createWorkspaceNavigation,
	navigateWorkspace,
} from "../src/app/svelte/workspaceNavigation";
import type { IncidentCockpitItem } from "../src/lib/types";
import { createWorkspaceRecord } from "../src/lib/workspace-model";

const item: IncidentCockpitItem = {
	resource: {
		kind: "Pod",
		cluster: "kind-dev",
		name: "api-7c9",
		namespace: "default",
		age: "5m",
		health: "unhealthy",
		status: "CrashLoopBackOff",
		ready: "0/1",
		restarts: 4,
	},
	severity: "attention",
	signals: [
		{ kind: "container", label: "Waiting", message: "CrashLoopBackOff", source: "pod" },
		{ kind: "restart", label: "Restarts", message: "4 restarts", source: "pod" },
		{ kind: "ready", label: "Ready", message: "0/1 ready", source: "pod" },
		{ kind: "event", label: "Warning", message: "Back-off", source: "event" },
	],
	warningEventCount: 2,
	latestSignalAt: "2026-06-04T10:02:00Z",
	latestWarningEvent: {
		eventType: "Warning",
		reason: "BackOff",
		message: "Back-off restarting failed container",
		count: 7,
		lastSeen: "2m",
		source: "kubelet",
		namespace: "default",
	},
};

function incidentSurfaceSource(): string {
	return [
		readFileSync(new URL("../src/app/svelte/AppSurfaces.svelte", import.meta.url), "utf8"),
		readFileSync(new URL("../src/app/svelte/IncidentSurface.svelte", import.meta.url), "utf8"),
	].join("\n");
}

describe("svelte incident surface model", () => {
	test("builds labeled filter options with counts", () => {
		expect(
			buildIncidentFilterOptions({
				total: 4,
				degraded: 1,
				attention: 1,
				restarted: 1,
				warning: 1,
			}),
		).toEqual([
			{ id: "all", label: "All", count: 4 },
			{ id: "unhealthy", label: "Unhealthy", count: 2 },
			{ id: "degraded", label: "Degraded", count: 1 },
			{ id: "attention", label: "Needs attention", count: 1 },
			{ id: "restarted", label: "Restarted", count: 1 },
			{ id: "warning", label: "Warnings", count: 1 },
		]);
	});

	test("maps incident resource jumps without hiding warning-only rows", () => {
		expect(incidentResourcesHealthFilter("all")).toBe("all");
		expect(incidentResourcesHealthFilter("warning")).toBe("all");
		expect(incidentResourcesHealthFilter("unhealthy")).toBe("unhealthy");
		expect(incidentResourcesHealthFilter("degraded")).toBe("degraded");
		expect(incidentResourcesHealthFilter("attention")).toBe("attention");
		expect(incidentResourcesHealthFilter("restarted")).toBe("restarted");
	});

	test("formats incident signal labels for the workbench", () => {
		expect(incidentSeverityLabel(item)).toBe("Needs attention");
		expect(incidentScopeLabel(item)).toBe("default / kind-dev");
		expect(incidentSignalSummary(item)).toBe(
			"Waiting: CrashLoopBackOff | Restarts: 4 restarts | Ready: 0/1 ready | +1 more",
		);
		expect(incidentWarningSummary(item)).toBe("BackOff (2m, 2 warnings)");
	});

	test("matches selected resources by stable incident key", () => {
		expect(incidentItemKey(item)).toBe("kind-dev::Pod:default:api-7c9");
		expect(isIncidentResourceSelected(item, item.resource)).toBe(true);
		expect(
			isIncidentResourceSelected(item, { ...item.resource, name: "api-other" }),
		).toBe(false);
		expect(isIncidentResourceSelected(item, null)).toBe(false);
	});

	test("derives selected incident case copy and detail pivots", () => {
		expect(incidentCaseTitle(item)).toBe("Pod/api-7c9: Waiting");
		expect(incidentCaseSummary(item)).toBe("CrashLoopBackOff");
		expect(incidentKnownSummary(item)).toBe(
			"CrashLoopBackOff, Ready 0/1, 4 restarts",
		);
		expect(incidentMissingSummary(item)).toBe(
			"Check Events for repeat count and source details.",
		);
		expect(incidentNextSummary(item)).toBe("Inspect details, then Events or Logs.");
		expect(incidentDetailPivots(item)).toEqual([
			{ id: "details", label: "Inspect", tab: "details", enabled: true },
			{ id: "events", label: "Events", tab: "events", enabled: true },
			{ id: "logs", label: "Logs", tab: "logs", enabled: true },
			{ id: "yaml", label: "YAML", tab: "yaml", enabled: true },
		]);
	});

	test("does not silently cap grouped incident rows", () => {
		const source = incidentSurfaceSource();

		expect(source).not.toContain("group.items.slice");
	});

	test("Svelte incidents distinguish clean scope from empty filter matches", () => {
		const source = incidentSurfaceSource();

		expect(source).toContain("incidentCounts.total === 0");
		expect(source).toContain("No active incident signals");
		expect(source).toContain("No matching incident signals");
		expect(source).toContain("Change severity filter to see other active signals.");
	});

	test("Svelte incidents wait for explicit signal selection", () => {
		const source = incidentSurfaceSource();

		expect(source).toContain("selectedResource = null");
		expect(source).toContain("Choose an incident signal");
		expect(source).toContain(
			"visibleIncidents.find((item) => isIncidentResourceSelected(item, selectedResource))",
		);
		expect(source).not.toContain("incidentGroups[0]");
	});

	test("Svelte incidents render queue and case workbench instead of a warning table", () => {
		const source = incidentSurfaceSource();

		expect(source).toContain("Signal queue");
		expect(source).toContain("Read-only pivots");
		expect(source).toContain("Open in Resources");
		expect(source).not.toContain("<Table");
		expect(source).not.toContain("Latest warning");
	});

	test("Svelte incidents use shared cockpit query key", () => {
		const source = readFileSync(
			new URL("../src/app/svelte/AppSurfaces.svelte", import.meta.url),
			"utf8",
		);

		expect(source).toContain("buildWorkspaceFetchKeys(workspace.scope)");
		expect(source).toContain("queryKeys.incidentCockpit(");
		expect(source).not.toContain('"svelte-incidents-surface"');
	});

	test("Svelte incident refresh shows fetching feedback", () => {
		const source = incidentSurfaceSource();

		const refreshStart = source.indexOf("onclick={() => incidentsQuery.refetch()}");
		const refreshEnd = source.indexOf("Refresh", refreshStart);
		const refreshBody = source.slice(refreshStart, refreshEnd);

		expect(refreshBody).toContain("incidentsQuery.isFetching");
		expect(refreshBody).toContain('<Spinner data-icon="inline-start" />');
		expect(refreshBody).toContain('<RotateCcw data-icon="inline-start" />');
	});

	test("Svelte incident inspection clears stale detail tab state for plain inspect", () => {
		const shell = readFileSync(
			new URL("../src/app/svelte/WorkspaceShell.svelte", import.meta.url),
			"utf8",
		);

		expect(shell).toContain(
			"resourceDetailPathState = detailTab ? detailPathStateForTab(detailTab) : null;",
		);
		expect(shell).not.toContain("if (detailTab) resourceDetailPathState = detailPathStateForTab(detailTab);");
	});

	test("Svelte overview incident shortcuts open Cockpit with matching filters", () => {
		const overview = readFileSync(
			new URL("../src/features/workspaces/WorkspaceOverview.svelte", import.meta.url),
			"utf8",
		);
		const surfaces = incidentSurfaceSource();
		const workspace = createWorkspaceRecord({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: [],
		});
		const shortcut = navigateWorkspace(createWorkspaceNavigation(workspace), {
			type: "openIncidents",
			filter: "restarted",
		});
		const sidebar = navigateWorkspace(shortcut, {
			type: "selectNode",
			node: { type: "section", section: "incidents" },
		});

		expect(overview).toContain(
			'IncidentShortcutButton("Unhealthy", unhealthyCount, "unhealthy", onOpenIncidents)',
		);
		expect(overview).toContain(
			'IncidentShortcutButton("Warnings", health.attention, "attention", onOpenIncidents)',
		);
		expect(overview).toContain(
			'IncidentShortcutButton("Restarted", health.restarted, "restarted", onOpenIncidents)',
		);
		expect(shortcut.initialIncidentFilter).toBe("restarted");
		expect(sidebar.initialIncidentFilter).toBe("all");
		expect(surfaces).toContain('initialIncidentFilter = "all"');
		expect(surfaces).toContain(
			'if (viewMode === "incidents") incidentFilter = initialIncidentFilter',
		);
	});
});
