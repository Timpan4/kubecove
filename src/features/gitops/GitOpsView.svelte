<script lang="ts">
	import { CircleHelp, GitBranch, Layers, Package, Plug } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Badge,
		Button,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		SegmentedControl,
		Tooltip,
		TooltipContent,
		TooltipProvider,
		TooltipTrigger,
	} from "@/components/ui/svelte";
	import { formatStatusLabel } from "@/lib/utils";
	import { settingsStore } from "@/lib/settings-store";
	import type { GitOpsViewMode } from "@/lib/settings";
	import type { ResourceSummary } from "@/lib/types";
	import {
		gitOpsDetailsActionKey,
		buildGitOpsSummary,
		gitOpsSelectionAgeTooltip,
		gitOpsSelectionKey,
		gitOpsSelectionPrimaryAction,
		gitOpsSelectionResource,
		gitOpsSelectionSourceLabel,
		gitOpsSelectionSourceLine,
		gitOpsSelectionSourceMode,
		gitOpsSelectionSourceTooltip,
		gitOpsSelectionSourceTooltipGroups,
		gitOpsSelectionSourceTooltipTitle,
		gitOpsSelectionRevisionLabel,
		gitOpsSelectionRevisionTooltipRows,
		gitOpsSelectionRevisionTooltipTitle,
		type GitOpsRevisionTooltipRow,
		type GitOpsSourceMode,
		type GitOpsSourceTooltipGroup,
		type GitOpsTooltipField,
		type GitOpsRailItem,
		type GitOpsSelection,
		type GitOpsData,
		type GitOpsTable,
		type GitOpsUnavailableProvider,
	} from "./surfaceModel";
	import SurfaceFrame from "@/components/SurfaceFrame.svelte";

	const gitOpsFactFieldClass =
		"relative min-w-0 rounded-md border bg-background/30 px-2.5 pb-2 pt-3 text-left";
	const gitOpsFactLabelClass =
		"pointer-events-none absolute -top-1.5 left-2 bg-surface-1 px-1 text-[0.62rem] font-medium leading-none text-muted-foreground";
	const gitOpsFactValueClass = "block min-w-0 truncate text-left font-medium";
	const gitOpsTooltipClass =
		"max-w-[34rem] flex-col items-stretch gap-2 border border-border/70 bg-surface-2 p-2 text-popover-foreground shadow-xl";
	const gitOpsTooltipHeaderClass =
		"text-[0.65rem] font-semibold uppercase tracking-wide text-muted-foreground";
	const gitOpsTooltipRowClass =
		"rounded-md border bg-background/40 px-2 py-1.5 text-xs leading-snug text-foreground";
	const gitOpsTooltipFieldGridClass =
		"grid grid-cols-[4.5rem_minmax(0,1fr)] gap-x-2 gap-y-1";
	const gitOpsSourceLineClass =
		"mt-3 w-full truncate rounded-md border bg-background/40 px-2.5 py-2 text-left text-xs text-muted-foreground transition-colors";

	type GitOpsCardFact = {
		label: string;
		value: string;
		revisionRows?: GitOpsRevisionTooltipRow[];
		tooltipFields?: GitOpsTooltipField[];
		tooltipTitle?: string;
	};

	type MaybeGitOpsCardFact = Omit<GitOpsCardFact, "value"> & {
		value: string | null | undefined;
	};

	type GitOpsQuery = {
		data?: GitOpsData;
		isPending: boolean;
		isError: boolean;
		error: unknown;
	};

	let {
		gitOpsQuery,
		gitOpsProviderError,
		gitOpsListError,
		gitOpsUnavailableProvider,
		gitOpsTable,
		gitOpsSelections,
		gitOpsRailItems,
		gitOpsActiveRailKey,
		selectedGitOpsItem = $bindable(null),
		selectedGitOpsItemKey,
		openSelectedArgoApplicationResources,
		onResourceInspect,
		gitOpsStatusClass,
	}: {
		gitOpsQuery: GitOpsQuery;
		gitOpsProviderError: unknown;
		gitOpsListError: unknown;
		gitOpsUnavailableProvider: GitOpsUnavailableProvider | null;
		gitOpsTable: GitOpsTable | null;
		gitOpsSelections: GitOpsSelection[];
		gitOpsRailItems: GitOpsRailItem[];
		gitOpsActiveRailKey: string;
		selectedGitOpsItem?: GitOpsSelection | null;
		selectedGitOpsItemKey: string;
		openSelectedArgoApplicationResources: (selection?: GitOpsSelection) => void;
		onResourceInspect: (resource: ResourceSummary) => void;
		gitOpsStatusClass: (status: string | null | undefined) => string;
	} = $props();

	const gitOpsViewOptions: { value: GitOpsViewMode; label: string }[] = [
		{ value: "cards", label: "Cards" },
		{ value: "list", label: "List" },
	];
	const gitOpsViewMode = $derived($settingsStore.gitOpsViewMode);
	const gitOpsSummary = $derived(
		gitOpsQuery.data ? buildGitOpsSummary(gitOpsQuery.data, gitOpsActiveRailKey) : null,
	);

	function railItemsFor(provider: GitOpsRailItem["provider"]): GitOpsRailItem[] {
		return gitOpsRailItems.filter((item: GitOpsRailItem) => item.provider === provider);
	}

	function openGitOpsSelection(selection: GitOpsSelection) {
		if (gitOpsSelectionPrimaryAction(selection) === "openResources") {
			selectedGitOpsItem = selection;
			openSelectedArgoApplicationResources(selection);
			return;
		}
		onResourceInspect(gitOpsSelectionResource(selection));
	}

	function gitOpsCardActionLabel(selection: GitOpsSelection): string {
		if (gitOpsSelectionPrimaryAction(selection) === "openResources") {
			return `View resources for ${selection.item.name}`;
		}
		return `Open details for ${selection.item.name}`;
	}

	function openGitOpsDetails(event: MouseEvent, selection: GitOpsSelection) {
		event.stopPropagation();
		onResourceInspect(gitOpsSelectionResource(selection));
	}

	function openGitOpsResources(event: MouseEvent, selection: GitOpsSelection) {
		event.stopPropagation();
		selectedGitOpsItem = selection;
		openSelectedArgoApplicationResources(selection);
	}

	function stopTooltipEvent(event: Event) {
		event.stopPropagation();
	}

	function handleGitOpsCardKeydown(event: KeyboardEvent, selection: GitOpsSelection) {
		if (event.key !== "Enter" && event.key !== " ") return;
		event.preventDefault();
		openGitOpsSelection(selection);
	}

	function gitOpsCardSubtitle(selection: GitOpsSelection): string {
		if (selection.type === "flux") return selection.item.resourceKind.kind;
		if (selection.type === "argoProject") return selection.item.description ?? "AppProject";
		return selection.item.project ?? "default";
	}

	function gitOpsSelectionDestination(selection: GitOpsSelection): string {
		if (selection.type === "flux" || selection.type === "argoProject") {
			return selection.item.namespace ?? "-";
		}
		return selection.item.destinationNamespace ?? selection.item.destinationServer ?? "-";
	}

	function gitOpsSummaryValueClass(tone: "healthy" | "unhealthy" | undefined): string {
		if (tone === "healthy") return "text-emerald-600 dark:text-emerald-300";
		if (tone === "unhealthy") return "text-destructive";
		return "text-foreground";
	}

	function gitOpsCardBadges(selection: GitOpsSelection): [string, string][] {
		if (selection.type === "flux") {
			return selection.item.readyStatus ? [["Ready", selection.item.readyStatus]] : [];
		}
		if (selection.type === "argoProject") {
			return selection.item.status ? [["Status", selection.item.status]] : [];
		}
		return [
			["Sync", selection.item.syncStatus],
			["Health", selection.item.healthStatus],
		].filter((entry): entry is [string, string] => Boolean(entry[1]));
	}

	function gitOpsCardFacts(selection: GitOpsSelection): GitOpsCardFact[] {
		if (selection.type === "flux") {
			const { item } = selection;
			return compactFacts([
				revisionFact(selection),
				{ label: "Namespace", value: item.namespace ?? "-" },
				{ label: "Inventory", value: String(item.inventory.length) },
				ageFact(selection),
			]);
		}
		if (selection.type === "argoProject") {
			const { item } = selection;
			return compactFacts([
				{ label: "Namespace", value: item.namespace ?? "-" },
				{ label: "Description", value: item.description ?? "-" },
				ageFact(selection),
			]);
		}
		const { item } = selection;
		return compactFacts([
			revisionFact(selection),
			{ label: "Destination", value: item.destinationNamespace ?? item.destinationServer ?? "-" },
			{ label: "Namespace", value: item.namespace ?? "-" },
			ageFact(selection),
		]);
	}

	function revisionFact(selection: GitOpsSelection): MaybeGitOpsCardFact {
		const rows = gitOpsSelectionRevisionTooltipRows(selection);
		return {
			label: "Revision",
			value: gitOpsSelectionRevisionLabel(selection),
			revisionRows: rows.length > 0 ? rows : undefined,
			tooltipTitle: rows.length > 0 ? gitOpsSelectionRevisionTooltipTitle(selection) : undefined,
		};
	}

	function ageFact(selection: GitOpsSelection): MaybeGitOpsCardFact {
		const createdAt = gitOpsSelectionAgeTooltip(selection);
		return {
			label: "Age",
			value: selection.item.age,
			tooltipTitle: createdAt ? "Age" : undefined,
			tooltipFields: createdAt ? [{ label: "created", value: createdAt }] : undefined,
		};
	}

	function compactFacts(rows: MaybeGitOpsCardFact[]): GitOpsCardFact[] {
		return rows.filter((row): row is GitOpsCardFact => Boolean(row.value));
	}

	function gitOpsCardTone(selection: GitOpsSelection): string {
		const values = gitOpsCardBadges(selection).map(([, value]) => value);
		if (values.some((value) => value === "Degraded" || value === "Missing" || value === "False")) {
			return "bg-destructive";
		}
		if (
			values.some((value) => value === "OutOfSync" || value === "Progressing" || value === "Unknown")
		) {
			return "bg-amber-400";
		}
		if (values.some((value) => value === "Synced" || value === "Healthy" || value === "True")) {
			return "bg-emerald-400";
		}
		if (values.some((value) => value === "Active" || value === "Ready")) {
			return "bg-emerald-400";
		}
		return "bg-muted";
	}

	function gitOpsSourceIcon(sourceMode: GitOpsSourceMode | null) {
		if (sourceMode === "git") return GitBranch;
		if (sourceMode === "helm") return Package;
		if (sourceMode === "multi") return Layers;
		if (sourceMode === "plugin") return Plug;
		if (sourceMode === "unknown") return CircleHelp;
		return null;
	}

	function gitOpsSourceIconClass(sourceMode: GitOpsSourceMode | null): string {
		const base =
			"inline-flex size-7 shrink-0 items-center justify-center rounded-md border bg-background/50";
		if (sourceMode === "git") return `${base} border-sky-500/35 text-sky-300`;
		if (sourceMode === "helm") return `${base} border-violet-500/35 text-violet-300`;
		if (sourceMode === "multi") return `${base} border-amber-500/35 text-amber-300`;
		if (sourceMode === "plugin") return `${base} border-emerald-500/35 text-emerald-300`;
		return `${base} border-border text-muted-foreground`;
	}

	function argoSourceCount(selection: GitOpsSelection): number | undefined {
		if (selection.type !== "argoApp") return undefined;
		const count = selection.item.sourceCount ?? selection.item.sources?.length;
		return count && count > 1 ? count : undefined;
	}

	function hasFactTooltip(fact: GitOpsCardFact): boolean {
		return Boolean(fact.revisionRows?.length || fact.tooltipFields?.length);
	}
</script>

{#snippet sourceTooltipContent(
	title: string,
	groups: GitOpsSourceTooltipGroup[],
	fallback: string,
)}
	<div class={gitOpsTooltipHeaderClass}>{title}</div>
	{#if groups.length > 0}
		{#each groups as group}
			<section class="space-y-1.5 rounded-md border bg-background/25 p-2">
				<div class="flex items-center justify-between gap-3 text-[0.68rem] font-semibold uppercase text-muted-foreground">
					<span>{group.label}</span>
					<span>{group.rows.length} source{group.rows.length === 1 ? "" : "s"}</span>
				</div>
				{#each group.rows as row}
					<div class={gitOpsTooltipRowClass}>
						<div class="mb-1 font-medium text-foreground">{row.name}</div>
						<div class={gitOpsTooltipFieldGridClass}>
							{#each row.fields as field}
								<span class="text-muted-foreground">{field.label}</span>
								<span class="break-all text-left font-medium">{field.value}</span>
							{/each}
						</div>
					</div>
				{/each}
			</section>
		{/each}
	{:else}
		<div class={gitOpsTooltipRowClass}>{fallback}</div>
	{/if}
{/snippet}

<TooltipProvider delayDuration={400} skipDelayDuration={0}>
	<SurfaceFrame icon={GitBranch} title="GitOps" query={gitOpsQuery} errorLabel="GitOps data unavailable" wide>
		{@const data = gitOpsQuery.data}
		{#if data}
			{#if gitOpsProviderError}
				<FriendlyError
					mode="compact"
					error={gitOpsProviderError}
					context={{
						operation: "providerDetection",
						fallbackTitle: "Some GitOps providers could not be detected",
						partial: true,
					}}
				/>
			{/if}
			{#if gitOpsListError}
				<FriendlyError
					mode="compact"
					error={gitOpsListError}
					context={{
						operation: "resourcesLoad",
						fallbackTitle: "Some GitOps resources could not load",
						partial: true,
					}}
				/>
			{/if}

			{#if gitOpsUnavailableProvider}
				<Empty>
					<EmptyHeader>
						<EmptyTitle>{gitOpsUnavailableProvider.title}</EmptyTitle>
						<EmptyDescription>{gitOpsUnavailableProvider.description}</EmptyDescription>
					</EmptyHeader>
				</Empty>
			{:else if !data.argoDetected && !data.fluxDetected}
				<Empty>
					<EmptyHeader>
						<EmptyTitle>No GitOps providers detected</EmptyTitle>
						<EmptyDescription>Argo CD and Flux resources were not detected in this cluster.</EmptyDescription>
					</EmptyHeader>
				</Empty>
			{:else if gitOpsTable}
				<div class="@container space-y-4">
					{#if gitOpsSummary}
						<section
							class="rounded-lg border bg-surface-1 px-3 py-2.5 shadow-sm"
							aria-label="GitOps provider overview"
						>
							<div class="flex flex-wrap items-start justify-between gap-3">
								<div class="min-w-0">
									<h2 class="font-heading text-base font-semibold">{gitOpsSummary.activeProvider}</h2>
									<div class="mt-2 flex flex-wrap gap-1.5">
										{#each gitOpsSummary.detectedProviders as provider}
											<Badge variant="outline" class="border-emerald-500/35 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300">
												{provider} detected
											</Badge>
										{/each}
									</div>
								</div>
								<div class="text-right">
									<div class="font-semibold tabular-nums">{gitOpsSummary.totalObjects}</div>
									<div class="text-xs text-muted-foreground">GitOps objects</div>
								</div>
							</div>
							<div class="mt-2.5 grid grid-cols-2 gap-x-4 gap-y-2 @min-[42rem]:grid-cols-5">
								{#each gitOpsSummary.facts as fact}
									<div class="min-w-0">
										<div class="text-[0.62rem] font-semibold uppercase tracking-wide text-muted-foreground">{fact.label}</div>
										<div class={`mt-0.5 font-semibold tabular-nums ${gitOpsSummaryValueClass(fact.tone)}`}>{fact.value}</div>
									</div>
								{/each}
							</div>
						</section>
					{/if}

					<div class="grid gap-4 @min-[64rem]:grid-cols-[14rem_minmax(0,1fr)]">
				<aside class="flex gap-5 overflow-x-auto border-b pb-3 @min-[64rem]:block @min-[64rem]:space-y-5 @min-[64rem]:overflow-visible @min-[64rem]:border-b-0 @min-[64rem]:border-r @min-[64rem]:pb-0 @min-[64rem]:pr-4">
					{#each ["Argo CD", "Flux"] as provider}
						{@const providerItems = railItemsFor(provider as GitOpsRailItem["provider"])}
						{#if providerItems.length > 0}
							<section class="flex shrink-0 items-center gap-2 @min-[64rem]:block @min-[64rem]:space-y-2">
								<p class="whitespace-nowrap px-1 text-xs font-semibold uppercase text-muted-foreground">{provider}</p>
								<div class="flex gap-1 @min-[64rem]:grid">
									{#each providerItems as item (item.key)}
										<div
											class={item.key === gitOpsActiveRailKey
												? "flex items-center justify-between gap-2 rounded-md bg-accent px-2.5 py-2 text-sm font-medium"
												: item.disabled
													? "flex items-center justify-between gap-2 rounded-md px-2.5 py-2 text-sm text-muted-foreground opacity-60"
													: "flex items-center justify-between gap-2 rounded-md px-2.5 py-2 text-sm text-muted-foreground"}
											aria-current={item.key === gitOpsActiveRailKey ? "true" : undefined}
										>
											<span class="min-w-0 truncate">{item.label}</span>
											<span class="rounded bg-muted px-1.5 py-0.5 text-xs leading-none tabular-nums">{item.count}</span>
										</div>
									{/each}
								</div>
							</section>
						{/if}
					{/each}
				</aside>

				<section class="min-w-0 space-y-4">
					<div class="flex flex-wrap items-end justify-between gap-3 border-b pb-3">
						<div>
							<h2 class="text-lg font-semibold">{gitOpsTable.title}</h2>
							<p class="text-sm text-muted-foreground">Delivery state, source, destination, and revision.</p>
						</div>
						<div class="flex items-center gap-2">
							<span class="text-xs text-muted-foreground tabular-nums">{gitOpsSelections.length} item{gitOpsSelections.length === 1 ? "" : "s"}</span>
							<SegmentedControl
								value={gitOpsViewMode}
								options={gitOpsViewOptions}
								onChange={$settingsStore.setGitOpsViewMode}
								ariaLabel="GitOps view mode"
							/>
						</div>
					</div>

					{#if gitOpsSelections.length === 0}
						<Empty>
							<EmptyHeader>
								<EmptyTitle>{gitOpsTable.empty}</EmptyTitle>
								<EmptyDescription>Select another GitOps provider or kind from the sidebar.</EmptyDescription>
							</EmptyHeader>
						</Empty>
					{:else}
						{#if gitOpsViewMode === "cards"}
							<div class="grid grid-cols-1 gap-3 @min-[35rem]:grid-cols-2 @min-[64rem]:grid-cols-[repeat(auto-fit,minmax(18.75rem,26.25rem))]">
							{#each gitOpsSelections as item (gitOpsSelectionKey(item))}
								{@const sourceMode = gitOpsSelectionSourceMode(item)}
								{@const SourceIcon = gitOpsSourceIcon(sourceMode)}
								{@const sourceLabel = gitOpsSelectionSourceLabel(sourceMode, argoSourceCount(item))}
								{@const sourceTooltip = gitOpsSelectionSourceTooltip(item)}
								{@const sourceTooltipTitle = gitOpsSelectionSourceTooltipTitle(item)}
								{@const sourceTooltipGroups = gitOpsSelectionSourceTooltipGroups(item)}
								{@const sourceLine = gitOpsSelectionSourceLine(item)}
								<div
									class={gitOpsSelectionKey(item) === selectedGitOpsItemKey
										? "group flex h-[18.5rem] cursor-pointer overflow-hidden rounded-lg border border-primary/60 bg-accent/40 text-xs/relaxed shadow-sm transition-all hover:-translate-y-px hover:border-primary/70 hover:bg-accent/50 hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 motion-reduce:transition-none motion-reduce:hover:translate-y-0"
										: "group flex h-[18.5rem] cursor-pointer overflow-hidden rounded-lg border bg-surface-1 text-xs/relaxed transition-all hover:-translate-y-px hover:border-primary/50 hover:bg-accent/30 hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 motion-reduce:transition-none motion-reduce:hover:translate-y-0"}
									role="button"
									tabindex="0"
									aria-label={gitOpsCardActionLabel(item)}
									aria-pressed={gitOpsSelectionKey(item) === selectedGitOpsItemKey}
									onclick={() => openGitOpsSelection(item)}
									onkeydown={(event: KeyboardEvent) => handleGitOpsCardKeydown(event, item)}
								>
									<div class={`w-1 shrink-0 ${gitOpsCardTone(item)}`}></div>
									<div class="flex min-w-0 flex-1 flex-col p-3">
										<div class="flex items-start justify-between gap-3">
											<div class="min-w-0 space-y-0.5">
												<div class="truncate font-heading text-sm font-medium">{item.item.name}</div>
												<div class="truncate text-xs/relaxed text-muted-foreground">{gitOpsCardSubtitle(item)}</div>
											</div>
											{#if SourceIcon}
												<Tooltip>
													<TooltipTrigger
														type="button"
														class={gitOpsSourceIconClass(sourceMode)}
														aria-label={sourceTooltip}
														onclick={stopTooltipEvent}
														onkeydown={stopTooltipEvent}
													>
														<SourceIcon class="size-3.5" aria-hidden="true" />
													</TooltipTrigger>
											<TooltipContent side="top" align="end" sideOffset={8} class={gitOpsTooltipClass}>
												{@render sourceTooltipContent(sourceTooltipTitle, sourceTooltipGroups, sourceLine ?? sourceLabel)}
											</TooltipContent>
												</Tooltip>
											{/if}
										</div>
										{#if gitOpsCardBadges(item).length > 0}
											<div class="mt-3 flex flex-wrap gap-1.5">
												{#each gitOpsCardBadges(item) as [, value]}
													<Badge variant="outline" class={gitOpsStatusClass(value)}>
														{formatStatusLabel(value)}
													</Badge>
												{/each}
											</div>
										{/if}
										{#if sourceLine}
											<Tooltip>
												<TooltipTrigger
													type="button"
													class={`${gitOpsSourceLineClass} hover:border-primary/40`}
													aria-label={sourceTooltip}
													onclick={stopTooltipEvent}
													onkeydown={stopTooltipEvent}
												>
													{sourceLine}
												</TooltipTrigger>
											<TooltipContent side="top" align="start" sideOffset={8} class={gitOpsTooltipClass}>
												{@render sourceTooltipContent(sourceTooltipTitle, sourceTooltipGroups, sourceLine)}
											</TooltipContent>
											</Tooltip>
										{/if}
									<div class="mt-3 grid grid-cols-2 gap-1.5">
											{#each gitOpsCardFacts(item) as fact}
												{#if hasFactTooltip(fact)}
													<Tooltip>
														<TooltipTrigger
															type="button"
															class={`${gitOpsFactFieldClass} w-full cursor-help transition-colors hover:border-primary/40`}
															aria-label={`${fact.label}: ${fact.value}`}
															onclick={stopTooltipEvent}
															onkeydown={stopTooltipEvent}
														>
															<span class={gitOpsFactLabelClass}>{fact.label}</span>
															<span class={gitOpsFactValueClass}>{fact.value}</span>
														</TooltipTrigger>
														<TooltipContent side="top" align="start" sideOffset={8} class={gitOpsTooltipClass}>
															<div class={gitOpsTooltipHeaderClass}>{fact.tooltipTitle ?? fact.label}</div>
															{#if fact.revisionRows && fact.revisionRows.length > 0}
																{#each fact.revisionRows as row}
																	<div class={gitOpsTooltipRowClass}>
																		<div class="mb-1 font-medium text-foreground">{row.name}</div>
																		<div class={gitOpsTooltipFieldGridClass}>
																			{#each row.fields as field}
																				<span class="text-muted-foreground">{field.label}</span>
																				<span class="break-all text-left font-medium">{field.value}</span>
																			{/each}
																		</div>
																	</div>
																{/each}
															{:else if fact.tooltipFields && fact.tooltipFields.length > 0}
																<div class={gitOpsTooltipRowClass}>
																	<div class={gitOpsTooltipFieldGridClass}>
																		{#each fact.tooltipFields as field}
																			<span class="text-muted-foreground">{field.label}</span>
																			<span class="break-all text-left font-medium">{field.value}</span>
																		{/each}
																	</div>
																</div>
															{/if}
														</TooltipContent>
													</Tooltip>
												{:else}
													<div class={gitOpsFactFieldClass}>
														<span class={gitOpsFactLabelClass}>{fact.label}</span>
														<span class={gitOpsFactValueClass}>{fact.value}</span>
													</div>
												{/if}
											{/each}
										</div>
										<div class="mt-auto flex flex-wrap items-center gap-2 pt-3">
											<Button
												type="button"
												variant="ghost"
												size="sm"
											data-details-key={gitOpsDetailsActionKey(item)}
											onclick={(event: MouseEvent) => openGitOpsDetails(event, item)}
											onkeydown={stopTooltipEvent}
											>
												Details
											</Button>
										{#if gitOpsSelectionPrimaryAction(item) === "openResources"}
											<Button
												type="button"
												variant="outline"
												size="sm"
											class="ml-auto"
											onclick={(event: MouseEvent) => openGitOpsResources(event, item)}
											onkeydown={stopTooltipEvent}
											>
												View resources
											</Button>
										{/if}
										</div>
									</div>
								</div>
							{/each}
							</div>
						{:else}
							<div class="overflow-x-auto rounded-lg border">
								<div
									class="grid min-w-[68rem] grid-cols-[minmax(12rem,1fr)_9rem_minmax(16rem,1.35fr)_minmax(7rem,0.6fr)_minmax(9rem,0.75fr)_4rem_9rem] items-center gap-3 border-b bg-muted/30 px-3 py-2 text-[0.65rem] font-semibold uppercase tracking-wide text-muted-foreground"
								>
									<span>Name</span>
									<span>Status</span>
									<span>Source(s)</span>
									<span>Revision</span>
									<span>Destination/Namespace</span>
									<span>Age</span>
									<span class="text-right">Actions</span>
								</div>
								{#each gitOpsSelections as item (gitOpsSelectionKey(item))}
									{@const sourceMode = gitOpsSelectionSourceMode(item)}
									{@const SourceIcon = gitOpsSourceIcon(sourceMode)}
									{@const sourceLabel = gitOpsSelectionSourceLabel(sourceMode, argoSourceCount(item))}
									{@const sourceTooltip = gitOpsSelectionSourceTooltip(item)}
									{@const sourceTooltipTitle = gitOpsSelectionSourceTooltipTitle(item)}
									{@const sourceTooltipGroups = gitOpsSelectionSourceTooltipGroups(item)}
									{@const sourceLine = gitOpsSelectionSourceLine(item)}
									<div
										class={gitOpsSelectionKey(item) === selectedGitOpsItemKey
											? "relative grid min-w-[68rem] grid-cols-[minmax(12rem,1fr)_9rem_minmax(16rem,1.35fr)_minmax(7rem,0.6fr)_minmax(9rem,0.75fr)_4rem_9rem] items-center gap-3 border-b bg-accent/40 px-3 py-2.5 text-sm transition-colors last:border-b-0 hover:bg-accent/50 motion-reduce:transition-none"
											: "relative grid min-w-[68rem] grid-cols-[minmax(12rem,1fr)_9rem_minmax(16rem,1.35fr)_minmax(7rem,0.6fr)_minmax(9rem,0.75fr)_4rem_9rem] items-center gap-3 border-b bg-surface-1 px-3 py-2.5 text-sm transition-colors last:border-b-0 hover:bg-accent/30 motion-reduce:transition-none"}
									>
										<div class={`absolute inset-y-0 left-0 w-1 ${gitOpsCardTone(item)}`}></div>
										<button
											type="button"
											class="min-w-0 rounded-sm pl-1 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
											aria-label={gitOpsCardActionLabel(item)}
											aria-pressed={gitOpsSelectionKey(item) === selectedGitOpsItemKey}
											onclick={() => openGitOpsSelection(item)}
										>
											<div class="truncate font-medium">{item.item.name}</div>
											<div class="truncate text-xs text-muted-foreground">{gitOpsCardSubtitle(item)}</div>
										</button>
										<div class="flex min-w-0 flex-wrap gap-1">
											{#each gitOpsCardBadges(item) as [, value]}
												<Badge variant="outline" class={gitOpsStatusClass(value)}>
													{formatStatusLabel(value)}
												</Badge>
											{/each}
										</div>
										{#if SourceIcon || sourceLine}
											<Tooltip>
												<TooltipTrigger
													type="button"
													class="flex min-w-0 items-center gap-2 text-left text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
													aria-label={sourceTooltip}
													onclick={stopTooltipEvent}
													onkeydown={stopTooltipEvent}
												>
													{#if SourceIcon}
														<span class={gitOpsSourceIconClass(sourceMode)} aria-hidden="true">
															<SourceIcon class="size-3.5" />
														</span>
													{/if}
													<span class="min-w-0 flex-1 truncate">{sourceLine ?? "-"}</span>
												</TooltipTrigger>
												<TooltipContent side="top" align="start" sideOffset={8} class={gitOpsTooltipClass}>
													{@render sourceTooltipContent(sourceTooltipTitle, sourceTooltipGroups, sourceLine ?? sourceLabel)}
												</TooltipContent>
											</Tooltip>
										{:else}
											<span class="text-muted-foreground">-</span>
										{/if}
										<span class="truncate font-medium">{gitOpsSelectionRevisionLabel(item)}</span>
										<span class="truncate text-muted-foreground">{gitOpsSelectionDestination(item)}</span>
										<span class="tabular-nums text-muted-foreground">{item.item.age ?? "-"}</span>
										<div class="flex justify-end gap-1">
											<Button
												type="button"
												variant="ghost"
												size="sm"
												data-details-key={gitOpsDetailsActionKey(item)}
												onclick={(event: MouseEvent) => openGitOpsDetails(event, item)}
												onkeydown={stopTooltipEvent}
											>
												Details
											</Button>
											{#if gitOpsSelectionPrimaryAction(item) === "openResources"}
												<Button
													type="button"
													variant="outline"
													size="sm"
													onclick={(event: MouseEvent) => openGitOpsResources(event, item)}
													onkeydown={stopTooltipEvent}
												>
													Resources
												</Button>
											{/if}
										</div>
									</div>
								{/each}
							</div>
						{/if}
					{/if}
				</section>
				</div>
				</div>
			{/if}
		{/if}
	</SurfaceFrame>
</TooltipProvider>
