import {
	countIncidentItems,
	filterIncidentItems,
	groupIncidentItems,
} from "./helpers";
import type { IncidentCockpitItem, IncidentSeverity } from "@/lib/types";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

function item(
	name: string,
	severity: IncidentSeverity,
	owner?: Partial<IncidentCockpitItem["resource"]>,
): IncidentCockpitItem {
	return {
		severity,
		signals: [],
		resource: {
			kind: "Pod",
			cluster: "admin@solid-k8s",
			name,
			namespace: "default",
			age: "1m",
			...owner,
		},
	};
}

describe("incident helpers", () => {
	test("counts incident severities", () => {
		const counts = countIncidentItems([
			item("api-0", "degraded"),
			item("api-1", "warning"),
			item("api-2", "warning"),
		]);

		expect(counts).toEqual({
			total: 3,
			degraded: 1,
			attention: 0,
			restarted: 0,
			warning: 2,
		});
	});

	test("filters by selected severity", () => {
		const rows = [
			item("api-0", "degraded"),
			item("api-1", "restarted"),
		];

		expect(filterIncidentItems(rows, "restarted")).toEqual([rows[1]]);
		expect(filterIncidentItems(rows, "all")).toEqual(rows);
	});

	test("groups by ownership context and sorts by severity", () => {
		const groups = groupIncidentItems([
			item("worker", "warning", { helmRelease: "jobs" }),
			item("api", "degraded", { argoApp: "payments" }),
			item("sidecar", "attention", { argoApp: "payments" }),
		]);

		expect(groups.length).toBe(2);
		expect(groups[0].label).toBe("Argo app: payments");
		expect(groups[0].items.map((row) => row.resource.name)).toEqual([
			"api",
			"sidecar",
		]);
		expect(groups[1].label).toBe("Helm release: jobs");
	});

	test("sorts equal severities by latest warning recency", () => {
		const older = item("api-a", "warning");
		older.latestWarningEvent = {
			eventType: "Warning",
			reason: "BackOff",
			message: "old warning",
			count: 1,
			lastSeen: "10m",
			lastSeenAt: "2026-06-04T10:00:00Z",
			source: "kubelet",
			namespace: "default",
		};
		const newer = item("api-z", "warning");
		newer.latestWarningEvent = {
			eventType: "Warning",
			reason: "FailedMount",
			message: "new warning",
			count: 1,
			lastSeen: "1m",
			lastSeenAt: "2026-06-04T10:05:00Z",
			source: "kubelet",
			namespace: "default",
		};

		const groups = groupIncidentItems([older, newer]);

		expect(groups[0].items.map((row) => row.resource.name)).toEqual([
			"api-z",
			"api-a",
		]);
	});
});
