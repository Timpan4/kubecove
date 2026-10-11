<script lang="ts">
	import {
		ArrowDown,
		ArrowDownUp,
		ArrowUp,
		ChevronDown,
		ChevronsUpDown,
		Pause,
		Play,
		RotateCcw,
		Search,
		WrapText,
	} from "lucide-svelte";
	import { SvelteSet } from "svelte/reactivity";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Button,
		buttonClass,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Input,
		Popover,
		PopoverContent,
		PopoverTrigger,
		SegmentedControl,
		Select,
		SelectContent,
		SelectGroup,
		SelectItem,
		SelectTrigger,
		SelectValue,
		Spinner,
		TabsContent,
	} from "@/components/ui/svelte";
	import type { LogLayout } from "@/lib/settings";
	import LogEntry from "./LogEntry.svelte";
	import type { ParsedLogLine } from "./log-helpers";
	import type { LogLevel } from "./log-structure";

	const sinceOptions = [
		{ value: "all", label: "Any time", seconds: undefined },
		{ value: "300", label: "5m", seconds: 300 },
		{ value: "900", label: "15m", seconds: 900 },
		{ value: "3600", label: "1h", seconds: 3600 },
	] as const;
	type SinceValue = (typeof sinceOptions)[number]["value"];

	const levelChips: { level: LogLevel; label: string; activeClass: string }[] = [
		{
			level: "error",
			label: "ERR",
			activeClass: "border-red-500/45 bg-red-500/10 text-red-600 dark:text-red-400",
		},
		{
			level: "warn",
			label: "WRN",
			activeClass: "border-amber-500/45 bg-amber-500/10 text-amber-700 dark:text-amber-400",
		},
		{
			level: "info",
			label: "INF",
			activeClass: "border-sky-500/45 bg-sky-500/10 text-sky-700 dark:text-sky-300",
		},
		{
			level: "debug",
			label: "DBG",
			activeClass: "border-border bg-muted text-foreground/80",
		},
	];

	let {
		isPod,
		isAggregate,
		containerOptions,
		selectedContainer = $bindable(""),
		logStatus,
		logMessage,
		logError,
		logFilter = $bindable(""),
		logFilterTerm,
		logWrapLines = $bindable(true),
		logLatestFirst = $bindable(false),
		logLayout,
		lineCount,
		onClear,
		logViewport = $bindable<HTMLElement | null>(null),
		logTailLines = $bindable(200),
		logSinceSeconds = $bindable<number | undefined>(undefined),
		logPaused,
		pendingCount,
		onTogglePause,
		unseenCount,
		onJumpToLatest,
		hiddenLevels,
		levelCounts,
		visibleLogLines,
		formatFullTimestamp,
		formatLogTime,
	}: {
		isPod: boolean;
		isAggregate: boolean;
		containerOptions: string[];
		selectedContainer: string;
		logStatus: string;
		logMessage: string;
		logError: unknown;
		logFilter: string;
		logFilterTerm: string;
		logWrapLines: boolean;
		logLatestFirst: boolean;
		logLayout: LogLayout;
		lineCount: number;
		onClear: () => void;
		logViewport: HTMLElement | null;
		logTailLines: number;
		logSinceSeconds?: number;
		logPaused: boolean;
		pendingCount: number;
		onTogglePause: () => void;
		unseenCount: number;
		onJumpToLatest: () => void;
		hiddenLevels: SvelteSet<LogLevel>;
		levelCounts: Record<LogLevel, number>;
		visibleLogLines: ParsedLogLine[];
		formatFullTimestamp: (timestamp: string | null | undefined) => string;
		formatLogTime: (timestamp: string | undefined) => string;
	} = $props();

	const openRows = new SvelteSet<string>();
	let expandAll = $state(false);
	let tailMenuOpen = $state(false);

	const sinceOption = $derived(
		sinceOptions.find((option) => option.seconds === logSinceSeconds) ?? sinceOptions[0],
	);
	const sinceValue = $derived(sinceOption.value);
	const sinceLabel = $derived(sinceOption.label);
	const tailLabel = $derived(
		`${logTailLines > 0 ? `Last ${logTailLines}` : "New only"} · ${sinceLabel}`,
	);
	const hasLevels = $derived(
		levelCounts.error + levelCounts.warn + levelCounts.info + levelCounts.debug > 0,
	);
	const filtering = $derived(Boolean(logFilterTerm) || hiddenLevels.size > 0);
	const statusLabel = $derived.by(() => {
		if (logPaused) return `Paused · ${pendingCount} buffered`;
		if (logStatus === "connected") return "Live";
		return logStatus.charAt(0).toUpperCase() + logStatus.slice(1);
	});
	const statusDotClass = $derived(
		logPaused
			? "bg-amber-500"
			: logStatus === "connected"
				? "bg-emerald-500"
				: logStatus === "error"
					? "bg-red-500"
					: "bg-muted-foreground/50",
	);
	const connecting = $derived(
		!logPaused && (logStatus === "connecting" || logStatus === "reconnecting"),
	);

	function setSinceSeconds(value: SinceValue) {
		logSinceSeconds = sinceOptions.find((option) => option.value === value)?.seconds;
	}

	function setTailLines(event: Event) {
		// SAFETY: this handler is attached only to the Tail numeric Input element.
		const value = Number((event.currentTarget as HTMLInputElement).value);
		logTailLines = Number.isFinite(value) ? Math.max(0, Math.floor(value)) : 0;
	}

	function toggleLevel(level: LogLevel) {
		if (hiddenLevels.has(level)) hiddenLevels.delete(level);
		else hiddenLevels.add(level);
	}

	function toggleRow(key: string) {
		if (openRows.has(key)) openRows.delete(key);
		else openRows.add(key);
	}

	function toggleExpandAll() {
		expandAll = !expandAll;
		openRows.clear();
	}

	function clearLines() {
		openRows.clear();
		onClear();
	}

	function rowKey(line: ParsedLogLine): string {
		return `${line.index}:${line.source?.podName ?? ""}:${line.source?.container ?? ""}:${line.raw}`;
	}
</script>

<TabsContent value="logs" class="flex min-h-0 flex-col">
	{#if !isPod && !isAggregate}
		<Empty class="min-h-32 border border-dashed">
			<EmptyHeader>
				<EmptyTitle>Logs unavailable</EmptyTitle>
				<EmptyDescription>Logs need a Pod, Deployment, or selector-backed Service.</EmptyDescription>
			</EmptyHeader>
		</Empty>
	{:else if isPod && containerOptions.length === 0}
		<Empty class="min-h-32 border border-dashed">
			<EmptyHeader>
				<EmptyTitle>No containers found</EmptyTitle>
				<EmptyDescription>Container status has not loaded for this Pod yet.</EmptyDescription>
			</EmptyHeader>
		</Empty>
	{:else}
		<div class="flex min-h-0 flex-1 flex-col gap-1.5">
			<div class="flex shrink-0 items-center gap-1.5">
				{#if isPod}
					<Select
						value={selectedContainer}
						items={containerOptions.map((container: string) => ({
							value: container,
							label: container,
						}))}
						onValueChange={(value: string) => (selectedContainer = value)}
					>
						<SelectTrigger class="h-7 w-40 max-w-64" aria-label="Log container">
							<SelectValue placeholder="Container" />
						</SelectTrigger>
						<SelectContent>
							<SelectGroup>
								{#each containerOptions as container (container)}
									<SelectItem value={container}>{container}</SelectItem>
								{/each}
							</SelectGroup>
						</SelectContent>
					</Select>
				{/if}
				<span class="inline-flex items-center gap-1.5 whitespace-nowrap text-xs" title={logMessage}>
					{#if connecting}
						<Spinner class="size-3" />
					{:else}
						<span class="size-[7px] shrink-0 rounded-full {statusDotClass}"></span>
					{/if}
					{statusLabel}
					<span class="text-muted-foreground">· {lineCount} lines</span>
				</span>
				<span class="flex-1"></span>
				<Popover bind:open={tailMenuOpen}>
					<PopoverTrigger
						type="button"
						class={buttonClass({ variant: "outline", className: "font-normal" })}
						aria-label="Log tail and time window"
					>
						{tailLabel}
						<ChevronDown data-icon="inline-end" />
					</PopoverTrigger>
					<PopoverContent align="end" class="flex w-60 flex-col gap-2.5 p-3">
						<label class="flex items-center justify-between gap-3 text-xs text-muted-foreground">
							Tail lines
							<Input
								class="h-7 w-24 text-xs"
								type="number"
								min="0"
								value={String(logTailLines)}
								oninput={setTailLines}
								aria-label="Tail log lines"
							/>
						</label>
						<div class="flex items-center justify-between gap-3 text-xs text-muted-foreground">
							Since
							<SegmentedControl
								value={sinceValue}
								options={sinceOptions.map((option) => ({
									value: option.value,
									label: option.value === "all" ? "Any" : option.label,
								}))}
								onChange={setSinceSeconds}
								ariaLabel="Log since window"
							/>
						</div>
					</PopoverContent>
				</Popover>
				<Button variant={logPaused ? "default" : "outline"} onclick={onTogglePause}>
					{#if logPaused}
						<Play data-icon="inline-start" />
						Resume
					{:else}
						<Pause data-icon="inline-start" />
						Pause
					{/if}
				</Button>
			</div>
			<div class="flex shrink-0 flex-wrap items-center gap-1.5">
				<div class="relative flex-[1_1_12rem]">
					<Search
						class="pointer-events-none absolute left-2 top-1/2 size-3 -translate-y-1/2 text-muted-foreground"
					/>
					<Input
						class="h-7 pl-6 pr-16 text-xs"
						bind:value={logFilter}
						placeholder="Filter logs..."
						aria-label="Filter log lines"
					/>
					{#if filtering}
						<span
							class="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-[11px] text-muted-foreground tabular-nums"
						>
							{visibleLogLines.length}/{lineCount}
						</span>
					{/if}
				</div>
				<div class="ml-auto flex shrink-0 items-center gap-1">
					{#if hasLevels}
						<div class="flex gap-1" role="group" aria-label="Filter by level">
							{#each levelChips as chip (chip.level)}
								{@const shown = !hiddenLevels.has(chip.level)}
								<Button
									variant="outline"
									class="gap-1.5 px-1.5 font-mono text-[11px] font-semibold {shown
										? chip.activeClass
										: 'text-muted-foreground/70'}"
									aria-pressed={shown}
									onclick={() => toggleLevel(chip.level)}
								>
									{chip.label}
									<span class="font-normal opacity-85">{levelCounts[chip.level]}</span>
								</Button>
							{/each}
						</div>
					{/if}
					{#if logLayout === "oneLine"}
						<Button
							variant="outline"
							size="icon"
							class={expandAll ? "bg-muted text-foreground" : "text-muted-foreground"}
							aria-pressed={expandAll}
							aria-label={expandAll ? "Collapse all" : "Expand all"}
							title={expandAll ? "Collapse all" : "Expand all"}
							onclick={toggleExpandAll}
						>
							<ChevronsUpDown />
						</Button>
					{:else}
						<Button
							variant="outline"
							size="icon"
							class={logWrapLines ? "bg-muted text-foreground" : "text-muted-foreground"}
							aria-pressed={logWrapLines}
							aria-label="Wrap lines"
							title="Wrap lines"
							onclick={() => (logWrapLines = !logWrapLines)}
						>
							<WrapText />
						</Button>
					{/if}
					<Button
						variant="outline"
						size="icon"
						class={logLatestFirst ? "bg-muted text-foreground" : "text-muted-foreground"}
						aria-pressed={logLatestFirst}
						aria-label={logLatestFirst ? "Latest first" : "Oldest first"}
						title={logLatestFirst ? "Latest first" : "Oldest first"}
						onclick={() => (logLatestFirst = !logLatestFirst)}
					>
						<ArrowDownUp />
					</Button>
					<Button
						variant="outline"
						size="icon"
						class="text-muted-foreground"
						aria-label="Clear"
						title="Clear"
						onclick={clearLines}
					>
						<RotateCcw />
					</Button>
				</div>
			</div>
			{#if logError}
				<FriendlyError
					mode="compact"
					error={logError}
					context={{
						operation: "partial",
						fallbackTitle: "Log stream failed",
						partial: true,
					}}
				/>
			{/if}
			<div class="relative flex min-h-0 flex-1 flex-col">
				<div
					bind:this={logViewport}
					class="relative min-h-0 flex-1 overflow-auto rounded-md border bg-background/50 [overflow-anchor:none]"
				>
					{#if lineCount === 0}
						<div class="flex items-center gap-2 p-3 text-sm text-muted-foreground">
							{#if connecting}<Spinner class="size-3.5" />{/if}
							<span>{logPaused ? "Paused. New lines are held until you resume." : logMessage || "Waiting for log lines..."}</span>
						</div>
					{:else if visibleLogLines.length === 0}
						<div class="p-3 text-sm text-muted-foreground">No log lines match the current filters.</div>
					{:else}
						{#each visibleLogLines as line (rowKey(line))}
							{@const key = rowKey(line)}
							<LogEntry
								{line}
								layout={logLayout}
								wrap={logWrapLines}
								open={expandAll || openRows.has(key)}
								term={logFilterTerm}
								levelColumn={hasLevels}
								showSource={isAggregate}
								onToggle={() => toggleRow(key)}
								{formatFullTimestamp}
								{formatLogTime}
							/>
						{/each}
					{/if}
				</div>
				{#if unseenCount > 0}
					<Button
						class="absolute left-1/2 z-10 -translate-x-1/2 rounded-full px-3 shadow-lg {logLatestFirst
							? 'top-3'
							: 'bottom-3'}"
						onclick={onJumpToLatest}
					>
						{#if logLatestFirst}<ArrowUp data-icon="inline-start" />{:else}<ArrowDown
								data-icon="inline-start"
							/>{/if}
						{unseenCount} new line{unseenCount === 1 ? "" : "s"}
					</Button>
				{/if}
			</div>
		</div>
	{/if}
</TabsContent>
