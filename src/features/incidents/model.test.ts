import type { IncidentCockpitItem, IncidentSeverity } from "@/lib/types";
import {
	buildIncidentFilterOptions,
	buildIncidentSurfaceState,
	countIncidentItems,
	filterIncidentItems,
	groupIncidentItems,
	type IncidentFilter,
	incidentCaseSummary,
	incidentGroupLabel,
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
		state: "active",
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
			"degraded",
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
			id: "degraded",
			label: "Degraded",
			count: 0,
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

		expect(incidentGroupLabel({ ...item("helm", "warning").resource, helmRelease: "payments" })).toBe(
			"Helm release: payments",
		);
		expect(groupIncidentItems([attention, older, newer])[0]?.label).toBe(
			"No GitOps ownership evidence",
		);
		expect(groupIncidentItems([attention, older, newer])[0]?.items).toEqual([
			newer,
			older,
			attention,
		]);
	});

	test("orders active warnings before historical restart evidence", () => {
		const historical = item("old-restart", "restarted", { state: "historical" });
		const warning = item("current-warning", "warning");

		expect(groupIncidentItems([historical, warning])[0]?.items).toEqual([
			warning,
			historical,
		]);
	});

	test("counts warning and restart evidence independently from canonical severity", () => {
		const mixed = item("api", "degraded", {
			warningEventCount: 1,
			signals: [
				{ kind: "restart", label: "Restart", message: "Container restarted", source: "pod", state: "active" },
				{ kind: "event", label: "FailedMount", message: "Volume unavailable", source: "kubelet", state: "active" },
			],
		});

		expect(countIncidentItems([mixed])).toEqual({
			total: 1,
			degraded: 1,
			attention: 0,
			restarted: 1,
			warning: 1,
		});
		expect(filterIncidentItems([mixed], "restarted")).toEqual([mixed]);
		expect(filterIncidentItems([mixed], "warning")).toEqual([mixed]);
	});

	test("exports the complete filter vocabulary", () => {
		const filters: IncidentFilter[] = [
			"all",
			"degraded",
			"attention",
			"restarted",
			"warning",
		];
		expect(filters).toHaveLength(5);
	});
});
