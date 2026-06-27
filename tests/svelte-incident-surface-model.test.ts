import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	buildIncidentFilterOptions,
	incidentResourcesHealthFilter,
	incidentScopeLabel,
	incidentSeverityLabel,
	incidentSignalSummary,
	incidentWarningSummary,
} from "../src/app/svelte/incidentSurfaceModel";
import type { IncidentCockpitItem } from "../src/lib/types";

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

	test("formats incident rows for compact Svelte tables", () => {
		expect(incidentSeverityLabel(item)).toBe("Needs attention");
		expect(incidentScopeLabel(item)).toBe("default / kind-dev");
		expect(incidentSignalSummary(item)).toBe(
			"Waiting: CrashLoopBackOff | Restarts: 4 restarts | Ready: 0/1 ready | +1 more",
		);
		expect(incidentWarningSummary(item)).toBe("BackOff (2m)");
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

	test("Svelte overview incident shortcuts open Cockpit with matching filters", () => {
		const overview = readFileSync(
			new URL("../src/features/workspaces/WorkspaceOverview.svelte", import.meta.url),
			"utf8",
		);
		const shell = readFileSync(
			new URL("../src/app/svelte/WorkspaceShell.svelte", import.meta.url),
			"utf8",
		);
		const surfaces = incidentSurfaceSource();

		expect(overview).toContain(
			'IncidentShortcutButton("Unhealthy", unhealthyCount, "unhealthy", onOpenIncidents)',
		);
		expect(overview).toContain(
			'IncidentShortcutButton("Warnings", health.attention, "attention", onOpenIncidents)',
		);
		expect(overview).toContain(
			'IncidentShortcutButton("Restarted", health.restarted, "restarted", onOpenIncidents)',
		);
		expect(shell).toContain("let initialIncidentFilter = $state<IncidentFilter>(");
		expect(shell).toContain('initialSurfacesPathState?.incidentFilter ?? "all"');
		expect(shell).toContain('function openIncidents(filter: IncidentFilter = "all")');
		expect(shell).toContain("initialIncidentFilter = filter");
		expect(shell).toContain(
			'nodeId.type === "section" && nodeId.section === "incidents" ? "all" : initialIncidentFilter',
		);
		expect(shell).toContain("{initialIncidentFilter}");
		expect(surfaces).toContain('initialIncidentFilter = "all"');
		expect(surfaces).toContain(
			'if (viewMode === "incidents") incidentFilter = initialIncidentFilter',
		);
	});
});
