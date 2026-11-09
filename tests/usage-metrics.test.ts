import { describe, expect, test } from "bun:test";
import {
	formatCpuPercent,
	formatMemoryBytes,
	formatProcessCount,
	formatUsageMetrics,
	formatUsageMetricsBreakdownDetails,
	flattenUsageMetricsBreakdown,
} from "../src/lib/usage-metrics";

describe("usage metrics formatting", () => {
	test("formats cpu as a clamped one-decimal percentage", () => {
		expect(formatCpuPercent(0)).toBe("0.0%");
		expect(formatCpuPercent(2.44)).toBe("2.4%");
		expect(formatCpuPercent(128)).toBe("100.0%");
		expect(formatCpuPercent(Number.NaN)).toBe("--");
	});

	test("formats memory with readable binary units", () => {
		expect(formatMemoryBytes(512)).toBe("512 B");
		expect(formatMemoryBytes(1536)).toBe("1.5 KB");
		expect(formatMemoryBytes(184 * 1024 * 1024)).toBe("184 MB");
		expect(formatMemoryBytes(2.5 * 1024 * 1024 * 1024)).toBe("2.5 GB");
	});

	test("formats process counts and full footer labels", () => {
		expect(formatProcessCount(Number.NaN)).toBe("--");
		expect(formatProcessCount(-1)).toBe("--");
		expect(formatProcessCount(0)).toBe("0 processes");
		expect(formatProcessCount(1)).toBe("1 process");
		expect(formatProcessCount(3)).toBe("3 processes");
		expect(
			formatUsageMetrics({
				cpuPercent: 2.4,
				memory: { wssBytes: null, rssBytes: 184 * 1024 * 1024, pssBytes: 120 * 1024 * 1024 },
				workingSetWindowMs: null,
				processCount: 3,
				sampledAt: "2026-05-20T10:00:00Z",
				breakdown: [],
			}),
		).toBe("CPU 2.4% · WSS -- · RSS 184 MB · PSS 120 MB · 3 processes");
		expect(
			formatUsageMetrics({
				cpuPercent: 0,
				memory: { wssBytes: 2.5 * 1024 * 1024 * 1024, rssBytes: null, pssBytes: null },
				workingSetWindowMs: null,
				processCount: 0,
				sampledAt: "2026-05-20T10:00:00Z",
				breakdown: [],
			}),
		).toBe("CPU 0.0% · WSS 2.5 GB · RSS -- · PSS -- · 0 processes");
	});

	test("formats nested usage breakdown rows", () => {
		const item = {
			label: "KubeCove",
			description: "Rust/Tauri host process",
			cpuPercent: 1.2,
			memory: { wssBytes: null, rssBytes: 256 * 1024 * 1024, pssBytes: null },
			processCount: 3,
			children: [
				{
					label: "WebView",
					description: "Renderer",
					cpuPercent: 0.8,
					memory: { wssBytes: null, rssBytes: 128 * 1024 * 1024, pssBytes: null },
					processCount: 2,
					children: [],
				},
			],
		};

		expect(formatUsageMetricsBreakdownDetails(item)).toBe(
			"CPU 1.2% · WSS -- · RSS 256 MB · PSS -- · 3 processes",
		);
		expect(formatUsageMetricsBreakdownDetails(item.children[0])).toBe(
			"CPU 0.8% · WSS -- · RSS 128 MB · PSS -- · 2 processes",
		);
		expect(flattenUsageMetricsBreakdown([item]).map(({ item, depth }) => [item.label, depth])).toEqual([
			["KubeCove", 0],
			["WebView", 1],
		]);
	});

	test("flattens sibling and deep usage rows in preorder", () => {
		const leaf = (label: string) => ({
			label,
			description: label,
			cpuPercent: 0,
			memory: { wssBytes: null, rssBytes: 0, pssBytes: null },
			processCount: 1,
			children: [],
		});
		const rows = [
			{ ...leaf("host"), children: [{ ...leaf("webview"), children: [leaf("renderer")] }] },
			leaf("sidecar"),
		];

		expect(flattenUsageMetricsBreakdown(rows).map(({ item, depth }) => [item.label, depth])).toEqual([
			["host", 0],
			["webview", 1],
			["renderer", 2],
			["sidecar", 0],
		]);
	});
});
