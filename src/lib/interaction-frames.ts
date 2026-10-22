// requestAnimationFrame deltas for the interaction recorder, in 0.1 ms buckets. Memory is
// bounded by the number of distinct deltas, not by how long a segment or session lasts.

export interface FrameHistogram {
	buckets: Map<number, number>;
	total: number;
}

export function emptyHistogram(): FrameHistogram {
	return { buckets: new Map(), total: 0 };
}

export function addFrame(histogram: FrameHistogram, deltaMs: number): void {
	const bucket = Math.round(deltaMs * 10);
	histogram.buckets.set(bucket, (histogram.buckets.get(bucket) ?? 0) + 1);
	histogram.total += 1;
}

/** Delta in ms and how many frames had it, shortest first. */
export function frameDeltas(histogram: FrameHistogram): [number, number][] {
	return [...histogram.buckets.entries()]
		.sort((a, b) => a[0] - b[0])
		.map(([bucket, count]) => [bucket / 10, count]);
}

export function percentileMs(histogram: FrameHistogram, fraction: number): number {
	let seen = 0;
	for (const [deltaMs, count] of frameDeltas(histogram)) {
		seen += count;
		if (seen > histogram.total * fraction) return deltaMs;
	}
	return 0;
}
