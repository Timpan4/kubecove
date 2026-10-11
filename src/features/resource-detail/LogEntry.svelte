<script lang="ts">
	import { Copy } from "lucide-svelte";
	import { Button } from "@/components/ui/svelte";
	import type { LogLayout } from "@/lib/settings";
	import type { ParsedLogLine } from "./log-helpers";
	import { highlightSegments, type LogLevel } from "./log-structure";

	const LEVEL_LABELS = {
		error: "ERR",
		warn: "WRN",
		info: "INF",
		debug: "DBG",
	} satisfies Record<LogLevel, string>;
	const LEVEL_BADGE_CLASSES = {
		error: "bg-red-600 text-white dark:bg-red-400 dark:text-red-950",
		warn: "bg-amber-500/15 text-amber-700 dark:text-amber-400",
		info: "text-sky-700 dark:text-sky-300",
		debug: "text-muted-foreground",
	} satisfies Record<LogLevel, string>;

	let {
		line,
		layout,
		wrap,
		open,
		term,
		levelColumn,
		showSource,
		onToggle,
		formatFullTimestamp,
		formatLogTime,
	}: {
		line: ParsedLogLine;
		layout: LogLayout;
		wrap: boolean;
		open: boolean;
		term: string;
		levelColumn: boolean;
		showSource: boolean;
		onToggle: () => void;
		formatFullTimestamp: (timestamp: string | null | undefined) => string;
		formatLogTime: (timestamp: string | undefined) => string;
	} = $props();

	let copied = $state(false);

	const structure = $derived(line.structure);
	const level = $derived(structure.level);
	const sourceLabel = $derived(
		!showSource || !line.source
			? ""
			: line.source.container
				? `${line.source.podName}/${line.source.container}`
				: line.source.podName,
	);
	const edgeClass = $derived(
		level === "error"
			? "border-l-red-500"
			: level === "warn"
				? "border-l-amber-500"
				: "border-l-transparent",
	);
	const clip = $derived(wrap ? "[overflow-wrap:anywhere]" : "truncate");

	function toggleFromRow() {
		// Keep text selection usable: finishing a drag-select should not collapse the row.
		if (window.getSelection()?.toString()) return;
		onToggle();
	}

	function onRowKeydown(event: KeyboardEvent) {
		if (event.target !== event.currentTarget) return;
		if (event.key !== "Enter" && event.key !== " ") return;
		event.preventDefault();
		onToggle();
	}

	async function copyLine(event: MouseEvent) {
		event.stopPropagation();
		try {
			await navigator.clipboard.writeText(line.raw);
			copied = true;
		} catch {
			// Clipboard access can be missing or denied; leave the button label unchanged.
			copied = false;
		}
	}
</script>

{#snippet highlighted(text: string)}{#each highlightSegments(text, term) as segment, index (index)}{#if segment.match}<mark class="rounded-sm bg-yellow-300/40 text-inherit">{segment.text}</mark>{:else}{segment.text}{/if}{/each}{/snippet}

{#snippet time()}
	<time
		class="min-w-[12ch] shrink-0 whitespace-nowrap font-mono text-[11px] leading-[17px] text-muted-foreground tabular-nums"
		datetime={line.timestamp}
		title={formatFullTimestamp(line.timestamp)}
	>
		{formatLogTime(line.timestamp)}
	</time>
{/snippet}

{#snippet badge()}
	{#if levelColumn}
		<span
			class="mt-[1.5px] inline-flex min-w-[30px] shrink-0 justify-center self-start rounded-[3px] px-1 font-mono text-[9.5px] font-bold leading-[14px] {level
				? LEVEL_BADGE_CLASSES[level]
				: ''}"
		>
			{level ? LEVEL_LABELS[level] : ""}
		</span>
	{/if}
{/snippet}

{#snippet source()}
	{#if sourceLabel}
		<span
			class="ml-auto max-w-[30%] shrink-0 truncate font-mono text-[10.5px] leading-[17px] text-muted-foreground"
			title={sourceLabel}
		>
			{sourceLabel}
		</span>
	{/if}
{/snippet}

{#snippet headline(extra: string)}
	<span class="min-w-0 text-[12.5px] font-medium leading-[17px] text-foreground {extra}">
		{@render highlighted(structure.headline)}
	</span>
{/snippet}

{#snippet errorText(extra: string)}
	{#if structure.error}
		<div class="font-mono text-[11.5px] leading-4 text-red-600 dark:text-red-300 {extra}">
			{@render highlighted(structure.error)}
		</div>
	{/if}
{/snippet}

{#snippet fieldChips()}
	{#if structure.fields.length > 0}
		<div class="flex flex-wrap gap-[3px]">
			{#each structure.fields as field, index (index)}
				<span
					class="whitespace-nowrap rounded border bg-muted/40 px-1.5 font-mono text-[10.5px] leading-[15px] text-foreground/85"
				>
					<span class="text-muted-foreground">{@render highlighted(field.key)}</span>
					{@render highlighted(field.value)}
				</span>
			{/each}
		</div>
	{/if}
{/snippet}

{#snippet fieldText(extra: string)}
	{#if structure.fields.length > 0}
		<div class="font-mono text-[11px] leading-4 text-foreground/85 {extra}">
			{#each structure.fields as field, index (index)}
				<span class="mr-2.5"
					><span class="text-muted-foreground">{@render highlighted(field.key)}</span>
					{@render highlighted(field.value)}</span
				>
			{/each}
		</div>
	{/if}
{/snippet}

{#if layout === "stacked"}
	<div
		data-log-row
		class="border-b border-l-2 border-b-border/40 px-2.5 py-1 hover:bg-muted/40 {edgeClass}">
		<div class="flex min-w-0 items-start gap-[7px]">
			{@render time()}
			{@render badge()}
			{@render headline(`flex-1 ${clip}`)}
			{@render source()}
		</div>
		{@render errorText(`mt-px ${clip}`)}
		{#if structure.fields.length > 0}
			<div class="mt-[3px]">
				{#if wrap}{@render fieldChips()}{:else}{@render fieldText("truncate")}{/if}
			</div>
		{/if}
	</div>
{:else if layout === "gutter"}
	<div
		data-log-row
		class="flex min-w-0 items-start gap-2 border-b border-l-2 border-b-border/30 py-[3px] pl-2.5 pr-2.5 hover:bg-muted/40 {edgeClass}"
	>
		{@render time()}
		{@render badge()}
		<div class="min-w-0 flex-1">
			<div class="flex min-w-0 items-start gap-2">
				{@render headline(`flex-1 ${clip}`)}
				{@render source()}
			</div>
			{@render errorText(clip)}
			{@render fieldText(clip)}
		</div>
	</div>
{:else}
	<div
		data-log-row
		role="button"
		tabindex="0"
		aria-expanded={open}
		class="flex min-w-0 cursor-pointer items-start gap-[7px] border-l-2 px-2.5 py-0.5 hover:bg-muted/40 {edgeClass} {open
			? 'bg-muted/50'
			: ''}"
		onclick={toggleFromRow}
		onkeydown={onRowKeydown}
	>
		{@render time()}
		{@render badge()}
		<div class="min-w-0 flex-1">
			<div class="text-[12px] leading-[18px] {open ? '[overflow-wrap:anywhere]' : 'truncate'}">
				{#if sourceLabel}<span class="mr-2 inline-block max-w-[30%] truncate align-bottom font-mono text-[10.5px] text-muted-foreground" title={sourceLabel}>{sourceLabel}</span>{/if}<span
					class="font-medium text-foreground">{@render highlighted(structure.headline)}</span
				>{#if !open && structure.error}<span
						class="ml-2 font-mono text-[11px] text-red-600 dark:text-red-300"
						>{@render highlighted(structure.error)}</span
					>{/if}{#if !open && structure.fields.length > 0}<span
						class="ml-2 font-mono text-[11px] text-foreground/75"
						>{#each structure.fields as field, index (index)}<span class="mr-2"
								><span class="text-muted-foreground">{@render highlighted(field.key)}</span>
								{@render highlighted(field.value)}</span
							>{/each}</span
					>{/if}
			</div>
			{#if open}
				<div class="flex flex-col items-start gap-1 pb-1 pt-0.5">
					{@render errorText("[overflow-wrap:anywhere]")}
					{@render fieldChips()}
					<Button variant="outline" size="xs" onclick={copyLine}>
						<Copy data-icon="inline-start" />
						{copied ? "Copied" : "Copy line"}
					</Button>
				</div>
			{/if}
		</div>
	</div>
{/if}
