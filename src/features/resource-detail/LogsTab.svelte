<script lang="ts">
	import { ArrowDownUp, Pause, Play, RotateCcw, Search } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Badge,
		Button,
		Checkbox,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Input,
		Label,
		Select,
		SelectContent,
		SelectGroup,
		SelectItem,
		SelectTrigger,
		SelectValue,
		Spinner,
		TabsContent,
	} from "@/components/ui/svelte";
	import type { LogLineEntry, ParsedLogLine } from "./log-helpers";

	const sinceOptions = [
		{ value: "all", label: "Any time", seconds: undefined },
		{ value: "300", label: "5m", seconds: 300 },
		{ value: "900", label: "15m", seconds: 900 },
		{ value: "3600", label: "1h", seconds: 3600 },
	] as const;

	let {
		isPod,
		isAggregate,
		targetKind,
		targetName,
		containerOptions,
		selectedContainer = $bindable(""),
		logStatus,
		logMessage,
		logError,
		logFilter = $bindable(""),
		logFilterTerm,
		logWrapLines = $bindable(true),
		logAutoFollow = $bindable(true),
		logLatestFirst = $bindable(false),
		logLines = $bindable<LogLineEntry[]>([]),
		logViewport = $bindable<HTMLElement | null>(null),
		logTailLines = $bindable(200),
		logSinceSeconds = $bindable<number | undefined>(undefined),
		logPaused = $bindable(false),
		visibleLogLines,
		parsedLogLines,
		formatFullTimestamp,
		formatLogTime,
	}: {
		isPod: boolean;
		isAggregate: boolean;
		targetKind: string;
		targetName: string;
		containerOptions: string[];
		selectedContainer: string;
		logStatus: string;
		logMessage: string;
		logError: unknown;
		logFilter: string;
		logFilterTerm: string;
		logWrapLines: boolean;
		logAutoFollow: boolean;
		logLatestFirst: boolean;
		logLines: LogLineEntry[];
		logViewport: HTMLElement | null;
		logTailLines: number;
		logSinceSeconds?: number;
		logPaused: boolean;
		visibleLogLines: ParsedLogLine[];
		parsedLogLines: ParsedLogLine[];
		formatFullTimestamp: (timestamp: string | null | undefined) => string;
		formatLogTime: (timestamp: string | undefined) => string;
	} = $props();

	const sinceValue = $derived(String(logSinceSeconds ?? "all"));
	const logTargetLabel = $derived(
		isPod ? `Pod/${targetName}` : `${targetKind}/${targetName} pods`,
	);

	function setSinceSeconds(value: string) {
		logSinceSeconds = sinceOptions.find((option) => option.value === value)?.seconds;
	}

	function setTailLines(event: Event) {
		const value = Number((event.currentTarget as HTMLInputElement).value);
		logTailLines = Number.isFinite(value) ? Math.max(0, Math.floor(value)) : 0;
	}

	function sourcePodName(line: ParsedLogLine): string {
		return line.source?.podName ?? "";
	}

	function sourceContainer(line: ParsedLogLine): string {
		return line.source?.container ?? "";
	}

	function sourceStyle(line: ParsedLogLine): string {
		const isContainer = sourceContainer(line).length > 0;
		return [
			`background-color: ${isContainer ? "hsl(154 58% 19% / 0.38)" : "hsl(142 56% 18% / 0.34)"}`,
			`border-color: ${isContainer ? "hsl(154 62% 45% / 0.48)" : "hsl(142 64% 45% / 0.48)"}`,
			`color: ${isContainer ? "hsl(154 78% 78%)" : "hsl(142 78% 78%)"}`,
		].join("; ");
	}

	function rowKey(line: ParsedLogLine): string {
		return `${line.index}:${sourcePodName(line)}:${sourceContainer(line)}:${line.raw}`;
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
		<div class="flex min-h-0 flex-1 flex-col gap-2">
			<div class="flex shrink-0 flex-wrap items-center gap-2">
				<Badge variant="outline" class="max-w-56 truncate">{logTargetLabel}</Badge>
				{#if isPod}
					<Select
						value={selectedContainer}
						items={containerOptions.map((container: string) => ({
							value: container,
							label: container,
						}))}
						onValueChange={(value: string) => (selectedContainer = value)}
					>
						<SelectTrigger class="h-8 w-44 max-w-64" aria-label="Log container">
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
				<Badge variant={logStatus === "error" ? "destructive" : "outline"}>{logStatus}</Badge>
				<span class="text-xs text-muted-foreground">{logMessage}</span>
				<Button
					variant={logPaused ? "default" : "outline"}
					size="sm"
					onclick={() => (logPaused = !logPaused)}
				>
					{#if logPaused}
						<Play data-icon="inline-start" />
						Resume
					{:else}
						<Pause data-icon="inline-start" />
						Pause
					{/if}
				</Button>
				<Label class="h-8 gap-1.5 rounded-md border bg-background px-2 text-xs text-muted-foreground">
					Tail
				<Input
					class="h-6 w-20 text-xs"
					type="number"
					min="0"
					value={String(logTailLines)}
					oninput={setTailLines}
					aria-label="Tail log lines"
				/>
				</Label>
				<Select value={sinceValue} onValueChange={setSinceSeconds}>
					<SelectTrigger class="h-8 w-28" aria-label="Log since window">
						<SelectValue />
					</SelectTrigger>
					<SelectContent>
						<SelectGroup>
							{#each sinceOptions as option (option.value)}
								<SelectItem value={option.value}>{option.label}</SelectItem>
							{/each}
						</SelectGroup>
					</SelectContent>
				</Select>
				<div class="relative min-w-40">
					<Search
						class="pointer-events-none absolute left-2 top-1/2 size-3 -translate-y-1/2 text-muted-foreground"
					/>
					<Input
						class="h-7 pl-6 text-xs"
						bind:value={logFilter}
						placeholder="Filter logs..."
						aria-label="Filter log lines"
					/>
				</div>
				{#if logFilterTerm}
					<Badge variant="outline">{visibleLogLines.length}/{parsedLogLines.length}</Badge>
				{/if}
				<Label class="h-7 gap-1.5 rounded-md border bg-background px-2 text-xs text-muted-foreground">
					<Checkbox
						class="size-3.5"
						checked={logWrapLines}
						onCheckedChange={(checked) => (logWrapLines = checked)}
					/>
					Wrap
				</Label>
				<Label class="h-7 gap-1.5 rounded-md border bg-background px-2 text-xs text-muted-foreground">
					<Checkbox
						class="size-3.5"
						checked={logAutoFollow}
						onCheckedChange={(checked) => (logAutoFollow = checked)}
					/>
					Auto-scroll
				</Label>
				<Button
					variant="outline"
					size="sm"
					aria-pressed={logLatestFirst}
					onclick={() => (logLatestFirst = !logLatestFirst)}
				>
					<ArrowDownUp data-icon="inline-start" />
					{logLatestFirst ? "Latest top" : "Oldest top"}
				</Button>
				<Button variant="outline" size="sm" onclick={() => (logLines = [])}>
					<RotateCcw data-icon="inline-start" />
					Clear
				</Button>
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
			<div
				bind:this={logViewport}
				class="min-h-0 flex-1 overflow-auto rounded-md border bg-background/50"
			>
				{#if logLines.length === 0}
					<div class="flex items-center gap-2 p-3 text-sm text-muted-foreground">
						{#if logStatus === "connecting"}<Spinner class="size-3.5" />{/if}
						<span>{logMessage || "Waiting for log lines..."}</span>
					</div>
				{:else if visibleLogLines.length === 0}
					<div class="p-3 text-sm text-muted-foreground">
						No log lines match "{logFilter.trim()}".
					</div>
				{:else}
				<div class="min-w-full font-mono text-xs leading-5 {logWrapLines ? '' : 'w-max'}">
					{#if isAggregate}
						<div class="sticky top-0 z-10 grid min-w-full grid-cols-[15rem_minmax(0,1fr)] border-b border-border bg-background text-[0.6875rem] uppercase text-muted-foreground">
							<div class="border-r border-border/50 px-3 py-1">Source</div>
							<div class="px-3 py-1">Message</div>
						</div>
					{/if}
					{#each visibleLogLines as line (rowKey(line))}
						{#if isAggregate && line.source}
							<div class="grid min-w-full grid-cols-[15rem_minmax(0,1fr)] items-start border-b border-border/50 last:border-b-0 {logWrapLines ? '' : 'w-max'}">
								<div
									class="min-w-0 space-y-1.5 border-r border-border/50 px-3 py-2"
									title={`${sourcePodName(line)} / ${sourceContainer(line)}`}
								>
									<time
										class="block h-4 whitespace-nowrap text-muted-foreground tabular-nums"
										datetime={line.timestamp}
										title={formatFullTimestamp(line.timestamp)}
									>
										{formatLogTime(line.timestamp)}
									</time>
									<Badge
										variant="outline"
										class="flex h-5 max-w-full justify-start truncate font-mono"
										style={sourceStyle(line)}
									>
										<span class="min-w-0 truncate">{sourcePodName(line)}</span>
									</Badge>
									<Badge
										variant="outline"
										class="flex h-5 max-w-full justify-start truncate font-mono"
										style={sourceStyle(line)}
									>
										<span class="min-w-0 truncate">{sourceContainer(line)}</span>
									</Badge>
								</div>
								<code
									class="block px-3 py-2 text-foreground {logWrapLines
										? 'min-w-0 whitespace-pre-wrap break-words'
										: 'whitespace-pre'}"
								>
									{line.message}
								</code>
							</div>
						{:else}
							<div class="flex min-w-full border-b border-border/50 last:border-b-0 {logWrapLines ? '' : 'w-max'}">
								<time
									class="w-32 shrink-0 whitespace-nowrap border-r border-border/50 px-3 py-1 text-muted-foreground tabular-nums"
									datetime={line.timestamp}
									title={formatFullTimestamp(line.timestamp)}
								>
									{formatLogTime(line.timestamp)}
								</time>
								{#if isAggregate && line.source}
									<span
										class="w-80 shrink-0 whitespace-nowrap border-r border-border/50 px-3 py-1 text-foreground"
										title={sourcePodName(line)}
									>
										{sourcePodName(line)}
									</span>
									<span
										class="w-44 shrink-0 truncate border-r border-border/50 px-3 py-1"
										title={sourceContainer(line)}
									>
										<Badge
											variant="outline"
											class="max-w-full truncate font-mono"
											style={sourceStyle(line)}
										>
											{sourceContainer(line)}
										</Badge>
									</span>
								{/if}
								<code
									class="block px-3 py-1 text-foreground {logWrapLines
										? 'min-w-0 flex-1 whitespace-pre-wrap break-words'
										: 'whitespace-pre'}"
								>
									{line.message}
								</code>
							</div>
						{/if}
					{/each}
				</div>
			{/if}
			</div>
		</div>
	{/if}
</TabsContent>
