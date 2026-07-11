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
		Tooltip,
		TooltipContent,
		TooltipProvider,
		TooltipTrigger,
	} from "@/components/ui/svelte";
	import { formatStatusLabel } from "@/lib/utils";
	import type { ResourceSummary } from "@/lib/types";
	import {
		gitOpsDetailsActionKey,
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
				<div class="grid gap-5 lg:grid-cols-[14rem_minmax(0,1fr)]">
				<aside class="space-y-5 border-r pr-4">
					{#each ["Argo CD", "Flux"] as provider}
						{@const providerItems = railItemsFor(provider as GitOpsRailItem["provider"])}
						{#if providerItems.length > 0}
							<section class="space-y-2">
								<p class="px-1 text-xs font-semibold uppercase text-muted-foreground">{provider}</p>
								<div class="grid gap-1">
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
					<div class="border-b pb-3">
						<h2 class="text-lg font-semibold">{gitOpsTable.title}</h2>
						<p class="text-sm text-muted-foreground">Read-only resource cards for detected GitOps providers.</p>
					</div>

					{#if gitOpsSelections.length === 0}
						<Empty>
							<EmptyHeader>
								<EmptyTitle>{gitOpsTable.empty}</EmptyTitle>
								<EmptyDescription>Select another GitOps provider or kind from the sidebar.</EmptyDescription>
							</EmptyHeader>
						</Empty>
					{:else}
						<div class="grid grid-cols-[repeat(auto-fill,minmax(19rem,1fr))] gap-3">
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
										? "group flex min-h-60 cursor-pointer overflow-hidden rounded-lg border border-primary/60 bg-accent/40 text-xs/relaxed shadow-sm transition-all hover:-translate-y-px hover:border-primary/70 hover:bg-accent/50 hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
										: "group flex min-h-60 cursor-pointer overflow-hidden rounded-lg border bg-surface-1 text-xs/relaxed transition-all hover:-translate-y-px hover:border-primary/50 hover:bg-accent/30 hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"}
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
														<div class={gitOpsTooltipHeaderClass}>{sourceTooltipTitle}</div>
														{#if sourceTooltipGroups.length > 0}
															{#each sourceTooltipGroups as group}
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
														{:else if sourceLine}
															<div class={gitOpsTooltipRowClass}>{sourceLine}</div>
														{:else}
															<div class={gitOpsTooltipRowClass}>{sourceLabel}</div>
														{/if}
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
													<div class={gitOpsTooltipHeaderClass}>{sourceTooltipTitle}</div>
													{#if sourceTooltipGroups.length > 0}
														{#each sourceTooltipGroups as group}
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
														<div class={gitOpsTooltipRowClass}>{sourceLine}</div>
													{/if}
												</TooltipContent>
											</Tooltip>
										{/if}
										<div class="mt-3 grid grid-cols-1 gap-1.5">
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
											>
												Details
											</Button>
											{#if gitOpsSelectionPrimaryAction(item) === "openResources"}
												<span class="ml-auto text-[0.68rem] text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100">
													Card opens resources
												</span>
											{/if}
										</div>
									</div>
								</div>
							{/each}
						</div>
					{/if}
				</section>
				</div>
			{/if}
		{/if}
	</SurfaceFrame>
</TooltipProvider>
