import { describe, expect, test } from "bun:test";
import type { PortForwardSessionSummary, ResourceSummary } from "../src/lib/types";
import {
	isPortForwardForResource,
	parsePortForwardForm,
	portForwardLocalUrl,
	sortPortForwardSessions,
} from "../src/features/live-sessions/helpers";

const baseSession: PortForwardSessionSummary = {
	id: "port-forward-1",
	clusterContext: "kind-dev",
	namespace: "payments",
	podName: "api-0",
	remotePort: 8080,
	localPort: 18080,
	localAddress: "127.0.0.1",
	localUrl: "http://127.0.0.1:18080",
	status: "listening",
	startedAt: "2026-05-31T00:00:00Z",
};

const baseResource: ResourceSummary = {
	kind: "Pod",
	cluster: "kind-dev",
	name: "api-0",
	namespace: "payments",
	age: "1m",
};

describe("port-forward helpers", () => {
	test("validates user-entered ports before command invocation", () => {
		expect(parsePortForwardForm({ remotePort: "8080", localPort: "" })).toEqual({
			remotePort: 8080,
		});
		expect(parsePortForwardForm({ remotePort: "8080", localPort: "18080" })).toEqual({
			remotePort: 8080,
			localPort: 18080,
		});
		expect(parsePortForwardForm({ remotePort: "", localPort: "" })).toBe(
			"Remote port must be a number",
		);
		expect(parsePortForwardForm({ remotePort: "70000", localPort: "" })).toBe(
			"Remote port must be between 1 and 65535",
		);
		expect(parsePortForwardForm({ remotePort: "8080", localPort: "80" })).toBe(
			"Local port must be 1024 or higher",
		);
	});

	test("matches active sessions to the selected Pod only", () => {
		expect(isPortForwardForResource(baseSession, baseResource)).toBe(true);
		expect(
			isPortForwardForResource(
				{ ...baseSession, podName: "worker-0" },
				baseResource,
			),
		).toBe(false);
		expect(
			isPortForwardForResource(baseSession, {
				...baseResource,
				kind: "Deployment",
			}),
		).toBe(false);
	});

	test("formats and sorts active sessions for stable UI", () => {
		const sessions = [
			{ ...baseSession, id: "b", podName: "worker-0", localPort: 18081 },
			{ ...baseSession, id: "a" },
		];

		expect(portForwardLocalUrl({ ...baseSession, localUrl: "" })).toBe(
			"http://127.0.0.1:18080",
		);
		expect(sortPortForwardSessions(sessions).map((session) => session.id)).toEqual([
			"a",
			"b",
		]);
	});
});
