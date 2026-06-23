<script lang="ts">
	import { ArrowDownUp, RotateCcw, Search } from "lucide-svelte";
	import {
		Alert,
		AlertDescription,
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

	let {
		isPod,
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
		logLines = $bindable([]),
		logViewport = $bindable(null),
		visibleLogLines,
		parsedLogLines,
		formatFullTimestamp,
		formatLogTime,
	} = $props();
</script>

<TabsContent value="logs">
		{#if !isPod}
			<Empty class="min-h-32 border border-dashed">
				<EmptyHeader>
					<EmptyTitle>Logs unavailable</EmptyTitle>
					<EmptyDescription>Pod logs require an exact Pod target.</EmptyDescription>
				</EmptyHeader>
			</Empty>
		{:else if containerOptions.length === 0}
			<Empty class="min-h-32 border border-dashed">
				<EmptyHeader>
					<EmptyTitle>No containers found</EmptyTitle>
					<EmptyDescription>Container status has not loaded for this Pod yet.</EmptyDescription>
				</EmptyHeader>
			</Empty>
		{:else}
			<div class="flex flex-col gap-2">
				<div class="flex flex-wrap items-center gap-2">
					{#if containerOptions.length > 1}
						<Select
							value={selectedContainer}
							items={containerOptions.map((container: string) => ({
								value: container,
								label: container,
							}))}
							onValueChange={(value: string) => (selectedContainer = value)}
						>
							<SelectTrigger class="h-8 max-w-64">
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
						Follow
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
					<Alert variant="destructive">
						<AlertDescription>{logError}</AlertDescription>
					</Alert>
				{/if}
				<div
					bind:this={logViewport}
					class="h-80 overflow-auto rounded-md border bg-background/50"
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
						<div
							class="min-w-full font-mono text-[11px] leading-5 {logWrapLines ? '' : 'w-max'}"
						>
							{#each visibleLogLines as line (`${line.index}:${line.raw}`)}
								<div class="flex min-w-full border-b border-border/50 last:border-b-0 {logWrapLines ? '' : 'w-max'}">
									<time
								class="w-32 shrink-0 whitespace-nowrap border-r border-border/50 px-3 py-1 text-muted-foreground tabular-nums"
								datetime={line.timestamp}
								title={formatFullTimestamp(line.timestamp)}
							>
								{formatLogTime(line.timestamp)}
							</time>
									<code
										class="block px-3 py-1 text-foreground {logWrapLines
											? 'min-w-0 flex-1 whitespace-pre-wrap break-words'
											: 'whitespace-pre'}"
									>
										{line.message}
									</code>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			</div>
		{/if}
	</TabsContent>
