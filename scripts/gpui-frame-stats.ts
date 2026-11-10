export type CpuFrame = {
	presentationOnly: boolean;
	cpu: number;
	draw: number;
	requestLayout: number;
	prepaint: number;
	paint: number;
	submit?: number;
};
export type Case = {
    gpuStep?: number[];
	cpuFrames?: CpuFrame[][];
	gpuCpuFrames?: CpuFrame[][];
	gpuScheduler?: ([number, number] | null)[];
	scheduler?: ([number, number] | null)[];
	id: string;
	kind: string;
	description: string;
	step?: number[];
	input?: number[];
	draw: number[];
	gpu: number[];
	gpuSize?: [number, number];
	changed: number;
	coldDraw: number;
};
export type Metric =
	| "frame"
	| "draw"
	| "step"
	| "gpu"
	| "inputWorst"
	| "presentation"
	| "gpuFrame"
	| "gpuPresentation"
	| "gpuWorst"
	| "threadCpu"
	| "runqueue"
	| "gpuSubmit"
	| "gpuThreadCpu"
	| "gpuRunqueue"
	| "gpuStep";
export type Stats = { p50: number; p95: number; p99: number; max: number };

/** Nearest-rank percentile in the samples' original units. */
function percentile(sorted: number[], p: number): number {
	return sorted[Math.max(0, Math.ceil((p / 100) * sorted.length) - 1)];
}

export type MetricStats = {
	median: Stats;
	maximum: number;
	p95Spread: [number, number];
	halves: [number, number];
};

export function stats(samples: number[], divisor = 1e6): Stats {
	const sorted = [...samples].sort((a, b) => a - b);
	return {
		p50: percentile(sorted, 50) / divisor,
		p95: percentile(sorted, 95) / divisor,
		p99: percentile(sorted, 99) / divisor,
		max: sorted[sorted.length - 1] / divisor,
	};
}
export function median(values: number[]): number {
	const sorted = [...values].sort((a, b) => a - b);
	const middle = Math.floor(sorted.length / 2);
	return sorted.length % 2
		? sorted[middle]
		: (sorted[middle - 1] + sorted[middle]) / 2;
}

export function aggregate(series: number[][], divisor = 1e6): MetricStats {
	const each = series.map((samples) => stats(samples, divisor));
	const key = (name: keyof Stats) => median(each.map((entry) => entry[name]));
	const halves = series.map((samples) => [
		stats(samples.slice(0, Math.ceil(samples.length / 2)), divisor).p50,
		stats(samples.slice(Math.floor(samples.length / 2)), divisor).p50,
	]);
	return {
		median: {
			p50: key("p50"),
			p95: key("p95"),
			p99: key("p99"),
			max: key("max"),
		},
		maximum: Math.max(...each.map((entry) => entry.max)),
		p95Spread: [
			Math.min(...each.map((entry) => entry.p95)),
			Math.max(...each.map((entry) => entry.p95)),
		],
		halves: [
			median(halves.map((half) => half[0])),
			median(halves.map((half) => half[1])),
		],
	};
}

export function frameCounts(
    entries: Case[],
    key: "cpuFrames" | "gpuCpuFrames",
    presentationOnly: boolean,
): MetricStats | undefined {
    const series: number[][] = [];
    for (const entry of entries) {
        const samples = entry[key];
        if (!samples?.length) return undefined;
        series.push(samples.map(frames => frames.filter(frame => frame.presentationOnly === presentationOnly).length));
    }
    return aggregate(series, 1);
}

/** Version 2 records every production-mode draw and its preceding update work. */
export function samplesOf(entry: Case, metric: Metric): number[] {
	if (metric === "gpuStep") return entry.gpuStep ?? [];
	if (metric === "gpuSubmit")
		return (
			entry.gpuCpuFrames?.flatMap((frames) =>
				frames.flatMap((frame) =>
					frame.submit === undefined ? [] : [frame.submit],
				),
			) ?? []
		);
	if (metric === "gpuThreadCpu" || metric === "gpuRunqueue")
		return entry.gpuScheduler?.every((pair) => pair !== null)
			? entry.gpuScheduler.flatMap((pair) =>
					pair ? [pair[metric === "gpuThreadCpu" ? 0 : 1]] : [],
				)
			: [];
	const step =
		entry.kind === "interaction" ? (entry.step ?? entry.input ?? []) : [];
	if (metric === "frame")
		return (
			entry.cpuFrames?.flatMap((frames) =>
				frames
					.filter((frame) => !frame.presentationOnly)
					.map((frame) => frame.cpu),
			) ?? (entry.kind === "interaction" ? step : entry.draw)
		);
	if (metric === "inputWorst")
		return (
			entry.cpuFrames?.map((frames) =>
				Math.max(...frames.map((frame) => frame.cpu)),
			) ?? samplesOf(entry, "frame")
		);
	if (metric === "draw")
		return (
			entry.cpuFrames?.flatMap((frames) =>
				frames
					.filter((frame) => !frame.presentationOnly)
					.map((frame) => frame.draw),
			) ?? entry.draw
		);
	if (metric === "presentation")
		return (
			entry.cpuFrames?.flatMap((frames) =>
				frames
					.filter((frame) => frame.presentationOnly)
					.map((frame) => frame.cpu),
			) ?? []
		);
	if (metric === "gpuFrame" || metric === "gpuPresentation")
		return (
			entry.gpuCpuFrames?.flatMap((frames) =>
				frames
					.filter((frame) => frame.presentationOnly === (metric === "gpuPresentation"))
					.map((frame) => frame.cpu),
			) ?? []
		);
	if (metric === "gpuWorst")
		return (
			entry.gpuCpuFrames?.map((frames) =>
				Math.max(...frames.map((frame) => frame.cpu)),
			) ?? []
		);
	if (metric === "threadCpu" || metric === "runqueue")
		return entry.scheduler?.every((pair) => pair !== null)
			? entry.scheduler.flatMap((pair) =>
					pair ? [pair[metric === "threadCpu" ? 0 : 1]] : [],
				)
			: [];
	if (metric === "step") return step;
	return entry.gpu;
}
