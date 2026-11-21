/**
 * A shard uploads its artifact as `<name>-attempt-<n>`. "Re-run failed jobs" keeps
 * earlier artifacts for shards it did not rerun. For rerun shards, only the newest
 * attempt represents the result.
 */
export function newestAttempts(names: string[]): string[] {
	const newest = new Map<string, { name: string; attempt: number }>();
	for (const name of names) {
		const match = /^(.*)-attempt-(\d+)$/.exec(name);
		const shard = match?.[1] ?? name;
		const attempt = Number(match?.[2] ?? 0);
		if (attempt >= (newest.get(shard)?.attempt ?? 0)) newest.set(shard, { name, attempt });
	}
	return [...newest.values()].map(({ name }) => name);
}
