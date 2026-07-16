import type { IncidentCockpitItem, IncidentSeverity } from "@/lib/types";
import {
	buildIncidentFilterOptions,
	buildIncidentSurfaceState,
	countIncidentItems,
	filterIncidentItems,
	groupIncidentItems,
	type IncidentFilter,
	incidentCaseSummary,
	incidentItemKey,
	incidentResourcesHealthFilter,
	reconcileIncidentSelection,
} from "./model";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toHaveLength(expected: number): void;
};

function item(
	name: string,
	severity: IncidentSeverity,
	overrides: Partial<IncidentCockpitItem> = {},
): IncidentCockpitItem {
	return {
		resource: {
			cluster: "kind-dev",
			kind: "Pod",
			name,
			namespace: "payments",
			age: "1h",
			health: severity === "warning" ? "attention" : severity,
		},
		severity,
		signals: [],
		warningEventCount: 0,
		...overrides,
	};
}

describe("incident presentation model", () => {
	test("builds the filtered grouped surface through one interface", () => {
		const degraded = item("api", "degraded", {
			latestSignalAt: "2026-07-12T10:00:00Z",
		});
		const restarted = item("worker", "restarted");
		const state = buildIncidentSurfaceState(
			[restarted, degraded],
			"unhealthy",
			incidentItemKey(degraded),
		);

		expect(state.counts).toEqual({
			total: 2,
			degraded: 1,
			attention: 0,
			restarted: 1,
			warning: 0,
		});
		expect(state.visibleCount).toBe(1);
		expect(state.groups[0]?.items).toEqual([degraded]);
		expect(state.selectedIncident).toEqual(degraded);
		expect(state.emptyState).toBe("ready");
	});

	test("selects highest severity then preserves or reconciles the local key", () => {
		const degraded = item("api", "degraded");
		const restarted = item("worker", "restarted");
		const restartedKey = incidentItemKey(restarted);

		expect(reconcileIncidentSelection([restarted, degraded], null)).toBe(incidentItemKey(degraded));
		expect(reconcileIncidentSelection([degraded, restarted], restartedKey)).toBe(restartedKey);
		expect(reconcileIncidentSelection([degraded], restartedKey)).toBe(incidentItemKey(degraded));
		expect(reconcileIncidentSelection([], restartedKey)).toBe(null);
	});

	test("owns filters counts options and resource handoff mapping", () => {
		const rows = [item("api", "attention"), item("worker", "warning")];
		const counts = countIncidentItems(rows);

		expect(filterIncidentItems(rows, "warning")).toEqual([rows[1]]);
		expect(buildIncidentFilterOptions(counts)[1]).toEqual({
			id: "unhealthy",
			label: "Unhealthy",
			count: 1,
		});
		expect(incidentResourcesHealthFilter("warning")).toBe("all");
		expect(incidentResourcesHealthFilter("attention")).toBe("attention");
	});

	test("preserves summary fallbacks without inventing signals", () => {
		const quiet = item("api", "warning");

		expect(incidentCaseSummary(quiet)).toBe(
			"Warning signal in payments / kind-dev.",
		);
		expect(incidentItemKey(quiet)).toBe("kind-dev::Pod:payments:api");
	});

	test("groups by ownership and orders by severity then recency", () => {
		const older = item("older", "degraded", {
			latestSignalAt: "2026-07-12T09:00:00Z",
		});
		const newer = item("newer", "degraded", {
			latestSignalAt: "2026-07-12T10:00:00Z",
		});
		const attention = item("attention", "attention");

		expect(groupIncidentItems([attention, older, newer])[0]?.items).toEqual([
			newer,
			older,
			attention,
		]);
	});

	test("exports the complete filter vocabulary", () => {
		const filters: IncidentFilter[] = [
			"all",
			"unhealthy",
			"degraded",
			"attention",
			"restarted",
			"warning",
		];
		expect(filters).toHaveLength(6);
	});
});
