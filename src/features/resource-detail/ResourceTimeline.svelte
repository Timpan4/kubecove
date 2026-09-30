<script lang="ts">
	import { ChevronRight, Clock } from "lucide-svelte";
	import { Button } from "@/components/ui/svelte";
	import { settingsStore } from "@/lib/settings-store";
	import type { IncidentTimelineItem, IncidentTimelineTone } from "./incident-timeline";
	import { incidentTimelineTitle } from "./incident-timeline-labels";

	let { items, resourceKind }: { items: IncidentTimelineItem[]; resourceKind: string } = $props();
	let expansion = $state<{ defaultOpen: boolean; overrides: Record<string, boolean> }>({
		defaultOpen: false,
		overrides: {},
	});
	const timezone = $derived($settingsStore.timestampTimezone);
	const expandByDefault = $derived($settingsStore.expandTimelineByDefault);
	const entries = $derived(items);
	const dateOptions = $derived({ timeZone: timezone === "utc" ? "UTC" : undefined });
	let now = $state(new Date());

	$effect(() => {
		expansion = { defaultOpen: expandByDefault, overrides: {} };
	});
	$effect(() => {
		const activeKeys = new Set(entries.map(entryKey));
		for (const key of Object.keys(expansion.overrides)) {
			if (!activeKeys.has(key)) delete expansion.overrides[key];
		}
	});
	$effect(() => {
		const midnight = new Date(now);
		if (timezone === "utc") midnight.setUTCHours(24, 0, 0, 0);
		else midnight.setHours(24, 0, 0, 0);
		const timer = window.setTimeout(() => now = new Date(), midnight.getTime() - now.getTime());
		return () => window.clearTimeout(timer);
	});

	function dateKey(date: Date): string {
		return date.toLocaleDateString("en-CA", { ...dateOptions, year: "numeric", month: "2-digit", day: "2-digit" });
	}

	const groups = $derived.by(() => {
		const result = new Map<string, { label: string; items: IncidentTimelineItem[] }>();
		for (const item of entries) {
			const date = item.timestamp ? new Date(item.timestamp) : null;
			const valid = date && !Number.isNaN(date.getTime());
			const key = valid ? dateKey(date) : "unknown";
			const group = result.get(key) ?? {
				label: valid ? date.toLocaleDateString(undefined, { ...dateOptions, day: "numeric", month: "short", year: "numeric" }) : "Time not reported",
				items: [],
			};
			group.items.push(item);
			result.set(key, group);
		}
		const unknown = result.get("unknown");
		result.delete("unknown");
		return [...Array.from(result, ([key, group]) => ({ key, ...group })), ...(unknown ? [{ key: "unknown", ...unknown }] : [])];
	});
	const todayKey = $derived(dateKey(now));
	const hideTodayHeading = $derived(
		groups.filter((group) => group.key !== "unknown").length === 1 &&
		groups.some((group) => group.key === todayKey),
	);

	function time(item: IncidentTimelineItem): string {
		if (!item.timestamp) return "No time";
		const date = new Date(item.timestamp);
		return Number.isNaN(date.getTime()) ? "No time" : date.toLocaleTimeString(undefined, { ...dateOptions, hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });
	}

	function toneClass(tone: IncidentTimelineTone): string {
		return { error: "text-destructive", warning: "text-amber-300", info: "text-sky-300", neutral: "text-secondary-foreground" }[tone];
	}

	function cardToneClass(tone: IncidentTimelineTone): string {
		return {
			error: "border-red-500/20 bg-red-500/5",
			warning: "border-amber-500/20 bg-amber-500/5",
			info: "border-sky-500/20 bg-sky-500/5",
			neutral: "border-border bg-background/30",
		}[tone];
	}

	function expandAll(open: boolean): void {
		expansion.overrides = Object.fromEntries(entries.map((item) => [entryKey(item), open]));
	}

	function entryKey(item: IncidentTimelineItem): string {
		return `${item.id}:${item.timestamp ?? ""}`;
	}
</script>

<section aria-label="Incident timeline" class="px-1 py-2">
	<div class="mb-3 flex flex-wrap items-center justify-between gap-2 border-b pb-2">
		<div class="flex items-center gap-2"><Clock class="size-3.5 text-muted-foreground" /><h2 class="text-xs font-semibold">Timeline</h2><span class="text-[0.6875rem] text-muted-foreground">{entries.length} {entries.length === 1 ? "entry" : "entries"}</span></div>
		<div class="flex items-center gap-1"><Button variant="ghost" size="sm" onclick={() => expandAll(true)}>Expand all</Button><Button variant="ghost" size="sm" onclick={() => expandAll(false)}>Collapse all</Button></div>
	</div>
	<p class="mb-2 text-[0.6875rem] text-muted-foreground">Oldest first{timezone === "utc" ? ", UTC" : ""}</p>
	{#if entries.length === 0}<p class="py-4 text-xs text-muted-foreground">No incident timeline entries for this resource.</p>
	{:else}
		{#each groups as group (group.key)}
			{#if group.label && !(hideTodayHeading && group.key === todayKey)}
				<div class="mb-1 mt-3 flex items-center gap-2 py-2 text-xs"><span class="font-medium">{group.label}</span><div class="h-px flex-1 bg-border/60"></div></div>
			{/if}
			<ol class="space-y-2.5">
				{#each group.items as item (entryKey(item))}
					{@const tone = item.source === "restart" ? "warning" : item.tone}
					<li class="relative grid grid-cols-[4rem_1rem_minmax(0,1fr)] grid-rows-[0.75rem_auto] items-start gap-x-2 before:absolute before:-bottom-2.5 before:left-20 before:top-0 before:w-px before:bg-border/60 first:before:top-7 last:before:bottom-auto last:before:h-7">
						<time datetime={item.timestamp} class="row-start-2 flex h-8 items-center justify-end font-mono text-[0.6875rem] leading-5 text-muted-foreground">{group.key === "unknown" ? "" : time(item)}</time>
						<span class="relative z-[var(--z-content)] row-start-2 flex h-8 items-center justify-center" aria-hidden="true"><span class={`size-2 rounded-full bg-current ${toneClass(tone)}`}></span></span>
						<fieldset data-timeline-card class={`col-start-3 row-span-2 row-start-1 min-w-0 rounded-lg border ${cardToneClass(tone)}`}>
							<legend data-timeline-source class={`ml-1.5 px-1.5 text-[0.6875rem] font-medium leading-3 capitalize ${toneClass(tone)}`}>{item.source}</legend>
							<details class="min-w-0" open={expansion.overrides[entryKey(item)] ?? expansion.defaultOpen} ontoggle={(event) => expansion.overrides[entryKey(item)] = event.currentTarget.open}>
								<summary aria-label={`${time(item)} ${incidentTimelineTitle(item, resourceKind)}`} class="grid cursor-pointer list-none grid-cols-[minmax(0,1fr)_0.75rem] items-start gap-x-2 rounded-lg px-3 py-1.5 text-xs hover:bg-white/[0.035] active:bg-white/5 focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-primary [&::-webkit-details-marker]:hidden">
									<span class="block min-w-0 break-words font-medium leading-5 text-foreground dark:text-white" title={incidentTimelineTitle(item, resourceKind)}>{incidentTimelineTitle(item, resourceKind)}</span>
									<ChevronRight class={`mt-1 size-3 text-foreground/60 ${(expansion.overrides[entryKey(item)] ?? expansion.defaultOpen) ? "rotate-90" : ""}`} />
								</summary>
								<div data-timeline-evidence class="mx-3 border-t border-current/10 pb-3 pt-2 text-xs text-foreground dark:text-white/90">
									{#if item.detail}<p class="whitespace-pre-wrap break-words leading-relaxed">{item.detail}</p>{/if}
									<dl class={`grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1.5 text-[0.6875rem] leading-4 ${item.detail ? "mt-3 border-t border-current/10 pt-3" : ""}`}>
										{#if item.source === "condition" && item.title.includes("=")}
											<dt class="text-foreground/65">Condition</dt><dd class="break-all font-medium">{item.title.split("=")[0]}</dd>
											<dt class="text-foreground/65">State</dt><dd class="font-medium">{item.title.split("=")[1]}</dd>
										{:else}<dt class="text-foreground/65">Reported as</dt><dd class="break-all font-medium">{item.title}</dd>{/if}
										<dt class="text-foreground/65">Timestamp</dt><dd class="break-all font-mono">{item.timestamp || "Not reported"}</dd>
									</dl>
								</div>
							</details>
						</fieldset>
					</li>
				{/each}
			</ol>
		{/each}
	{/if}
</section>
