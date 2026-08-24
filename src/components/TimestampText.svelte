<script lang="ts">
	import { settingsStore } from "@/lib/settings-store";
	import {
		formatExactTimestamp,
		formatRelativeTimestamp,
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
		formatExactTimestamp(value, $settingsStore.timestampTimezone, precision),
	);
	const display = $derived(
		relative
			? formatRelativeTimestamp(
					relative,
					value,
					$settingsStore.showExactTimestamps,
					$settingsStore.timestampTimezone,
					precision,
				)
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
