import { describe, expect, test } from "bun:test";
import type { IncidentCockpitItem } from "../src/lib/types";
import { createWorkspaceRecord } from "../src/lib/workspace-model";
import {
	buildIncidentFilterOptions,
	buildIncidentQueryState,
	buildIncidentSurfaceState,
	incidentCaseSummary,
	incidentCaseTitle,
	incidentItemKey,
	incidentResourcesHealthFilter,
	incidentScopeLabel,
	incidentSeverityLabel,
	incidentSignalSummary,
	reconcileIncidentSelection,
} from "../src/features/incidents/model";

const item: IncidentCockpitItem = {
	resource: {
		kind: "Pod",
		cluster: "kind-dev",
		name: "api-7c9",
		namespace: "default",
		age: "5m",
		health: "attention",
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

describe("incident surface model", () => {
	test("publishes scope-aware query state through the feature boundary", () => {
		const workspace = createWorkspaceRecord({
			name: "Ops",
			clusterContext: "kind-dev",
			clusterContexts: ["kind-prod"],
			namespaces: ["default"],
			kinds: ["Pod"],
		});
		const waiting = buildIncidentQueryState(workspace, false, "source-a");
		const ready = buildIncidentQueryState(workspace, true, "source-a");

		expect(waiting.enabled).toBe(false);
		expect(ready.enabled).toBe(true);
		expect(ready.fetchPlans).toEqual([
			{ clusterContext: "kind-dev", requests: [{ kind: "Pod", namespace: "default" }] },
			{ clusterContext: "kind-prod", requests: [{ kind: "Pod", namespace: "default" }] },
		]);
		expect(ready.queryKey).toContain("kubeconfigEnv=source-a");
	});

	test("publishes selection, empty states, and uncapped rows through the feature boundary", () => {
		const items = Array.from({ length: 120 }, (_, index) => ({
			...item,
			resource: { ...item.resource, name: `api-${index}` },
		}));
		const ready = buildIncidentSurfaceState(items, "attention", incidentItemKey(items[87]));
		const filtered = buildIncidentSurfaceState(items, "degraded", null);
		const clean = buildIncidentSurfaceState([], "all", null);

		expect(ready.visibleCount).toBe(120);
		expect(ready.groups.flatMap((group) => group.items)).toHaveLength(120);
		expect(ready.selectedIncident?.resource.name).toBe("api-87");
		expect(ready.emptyState).toBe("ready");
		expect(filtered.emptyState).toBe("filtered");
		expect(clean.emptyState).toBe("clean");
	});

	test("builds labeled filter options with counts", () => {
		expect(buildIncidentFilterOptions({
			total: 4,
			degraded: 1,
			attention: 1,
			restarted: 1,
			warning: 1,
		})).toEqual([
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
	});

	test("reconciles selected resources by stable incident key", () => {
		const key = incidentItemKey(item);
		expect(key).toBe("kind-dev::Pod:default:api-7c9");
		expect(reconcileIncidentSelection([item], key)).toBe(key);
		expect(reconcileIncidentSelection([item], "missing")).toBe(key);
		expect(reconcileIncidentSelection([], key)).toBeNull();
	});

	test("derives selected incident case copy", () => {
		expect(incidentCaseTitle(item)).toBe("Pod/api-7c9: Waiting");
		expect(incidentCaseSummary(item)).toBe("CrashLoopBackOff");
	});
});
