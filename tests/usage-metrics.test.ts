import { describe, expect, test } from "bun:test";
import {
	formatCpuPercent,
	formatMemoryBytes,
	formatProcessCount,
	formatUsageMetrics,
	formatUsageMetricsBreakdown,
	formatUsageMetricsBreakdownDetails,
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
				memoryBytes: 184 * 1024 * 1024,
				processCount: 3,
				sampledAt: "2026-05-20T10:00:00Z",
				breakdown: [],
			}),
		).toBe("CPU 2.4% · Memory 184 MB · 3 processes");
		expect(
			formatUsageMetrics({
				cpuPercent: 0,
				memoryBytes: 2.5 * 1024 * 1024 * 1024,
				processCount: 0,
				sampledAt: "2026-05-20T10:00:00Z",
				breakdown: [],
			}),
		).toBe("CPU 0.0% · Memory 2.5 GB · 0 processes");
	});

	test("formats usage breakdown rows", () => {
		const item = {
			label: "WebView",
			description: "Embedded WebView browser runtime",
			cpuPercent: 0.8,
			memoryBytes: 512 * 1024 * 1024,
			processCount: 5,
			children: [],
		};

		expect(formatUsageMetricsBreakdown(item)).toBe(
			"WebView · CPU 0.8% · 512 MB · 5 processes",
		);
		expect(formatUsageMetricsBreakdownDetails(item)).toBe(
			"CPU 0.8% · 512 MB · 5 processes",
		);
	});

	test("formats nested usage breakdown rows", () => {
		const item = {
			label: "KubeCove",
			description: "Rust/Tauri host process",
			cpuPercent: 1.2,
			memoryBytes: 256 * 1024 * 1024,
			processCount: 3,
			children: [
				{
					label: "WebView",
					description: "Renderer",
					cpuPercent: 0.8,
					memoryBytes: 128 * 1024 * 1024,
					processCount: 2,
					children: [],
				},
			],
		};

		expect(formatUsageMetricsBreakdown(item)).toBe(
			"KubeCove · CPU 1.2% · 256 MB · 3 processes",
		);
		expect(formatUsageMetricsBreakdown(item.children[0])).toBe(
			"WebView · CPU 0.8% · 128 MB · 2 processes",
		);
	});
});
