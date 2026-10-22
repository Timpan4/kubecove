<script module lang="ts">
	import { derived } from "svelte/store";
	import { settingsStore } from "@/lib/settings-store";
	const timezone = derived(settingsStore, (settings) => settings.timestampTimezone);
	const showExact = derived(settingsStore, (settings) => settings.showExactTimestamps);
</script>

<script lang="ts">
	import {
		formatExactTimestamp,
		type TimestampPrecision,
	} from "./timestamp-format";

	let {
		value,
		relative,
		precision = "minute",
		fallback = "—",
		class: className = "",
	}: {
		value?: string | null;
		relative?: string | null;
		precision?: TimestampPrecision;
		fallback?: string;
		class?: string;
	} = $props();

	const exactTimestamp = $derived(
		formatExactTimestamp(value, $timezone, precision),
	);
	const display = $derived(
		relative
			? ($showExact && exactTimestamp ? `${relative} (${exactTimestamp})` : relative)
			: (exactTimestamp ?? value ?? fallback),
	);
</script>

{#if value}
	<time
		datetime={value}
		title={value}
		aria-label={`${display}. Exact timestamp ${value}`}
		class={className}
	>{display}</time>
{:else}
	<span class={className}>{relative ?? fallback}</span>
{/if}
