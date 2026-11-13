<script lang="ts">
	import type { Snippet } from "svelte";
	import { SvelteSet } from "svelte/reactivity";
	import { ChevronDown, CircleCheck, CircleDashed, CircleHelp, CircleX, TriangleAlert } from "lucide-svelte";
	import CopyableText from "@/components/CopyableText.svelte";
	import LazyTooltip from "@/components/LazyTooltip.svelte";
	import VirtualCollection from "@/components/VirtualCollection.svelte";
	import { Badge, Button } from "@/components/ui/svelte";
	import type { HealthAssessmentState } from "@/lib/health-types";
	import { healthSourceSummary, healthStateLabel } from "@/lib/resource-health";
	import { formatStatusLabel } from "@/lib/utils";
	import {
		buildGitOpsListEntries,
		gitOpsListColumns,
		gitOpsListEntryKey,
		gitOpsListState,
		gitOpsListSync,
		type GitOpsListGroup,
		type GitOpsListSyncTone,
	} from "./surfaceList";
	import {
		gitOpsDetailsActionKey,
		gitOpsSelectionKey,
		gitOpsSelectionPrimaryAction,
		gitOpsSelectionRevisionLabel,
		gitOpsSelectionRevisionTooltipRows,
		gitOpsSelectionSourceLabel,
		gitOpsSelectionSourceLine,
		gitOpsSelectionSourceMode,
		gitOpsSelectionSourceTooltip,
		gitOpsSelectionSourceTooltipGroups,
		gitOpsSelectionSourceTooltipTitle,
		type GitOpsSelection,
		type GitOpsSourceMode,
		type GitOpsSourceTooltipGroup,
	} from "./surfaceModel";
	import {
		argoSourceCount,
		gitOpsCardActionLabel,
		gitOpsCardBadges,
		gitOpsCardSubtitle,
		gitOpsCardTone,
		gitOpsDetailsActionLabel,
		gitOpsSourceIcon,
		type GitOpsTrackedRequest,
		stopTooltipEvent,
	} from "./surfacePresentation";

	let {
		selections,
		selectedKey,
		statusClass,
		trackedRequest,
		tooltipClass,
		sourceTooltip,
		onOpen,
		onDetails,
		onResources,
		onCopy,
	}: {
		selections: GitOpsSelection[];
		selectedKey: string;
		statusClass: (status: string | null | undefined) => string;
		trackedRequest: (selection: GitOpsSelection) => GitOpsTrackedRequest | null;
		tooltipClass: string;
		sourceTooltip: Snippet<[string, GitOpsSourceTooltipGroup[], string]>;
		onOpen: (selection: GitOpsSelection) => void;
		onDetails: (event: MouseEvent, selection: GitOpsSelection) => void;
		onResources: (event: MouseEvent, selection: GitOpsSelection) => void;
		onCopy: (event: Event, value: string, label: string) => void;
	} = $props();

	// The minimum is the sum of the narrow tracks, gaps and padding. It fits the list panel of a
	// 1440px window (873px), so the list scrolls sideways only in smaller windows.
	// Actions holds Details alone below 68rem, Details and Resources above.
	const minWidth = "53.75rem";
	const columnsClass =
		"grid min-w-[53.75rem] grid-cols-[minmax(10.5rem,1.2fr)_7.5rem_5rem_minmax(7.5rem,2.2fr)_4.5rem_minmax(5.5rem,0.8fr)_2.5rem_4rem] items-center gap-3 px-3 @min-[68rem]:grid-cols-[minmax(10.5rem,1.2fr)_7.5rem_5rem_minmax(7.5rem,2.2fr)_4.5rem_minmax(5.5rem,0.8fr)_2.5rem_9rem]";
	const healthTooltipClass =
		"max-w-sm flex-col items-stretch rounded-lg border border-border bg-surface-2 p-3 text-popover-foreground shadow-xl";

	const healthPresentation = {
		healthy: { icon: CircleCheck, iconClass: "text-emerald-700 dark:text-emerald-300", textClass: "text-muted-foreground" },
		needsAttention: { icon: TriangleAlert, iconClass: "text-amber-700 dark:text-amber-300", textClass: "font-medium text-amber-700 dark:text-amber-300" },
		degraded: { icon: CircleX, iconClass: "text-destructive", textClass: "font-medium text-destructive" },
		unknown: { icon: CircleHelp, iconClass: "text-muted-foreground", textClass: "text-muted-foreground" },
		notEvaluated: { icon: CircleDashed, iconClass: "text-muted-foreground", textClass: "text-muted-foreground" },
	} satisfies Record<HealthAssessmentState, { icon: typeof CircleCheck; iconClass: string; textClass: string }>;
	const syncToneClass = {
		quiet: "text-muted-foreground",
		active: "font-medium text-foreground",
		attention: "font-medium text-amber-700 dark:text-amber-300",
		failed: "font-medium text-destructive",
	} satisfies Record<GitOpsListSyncTone, string>;

	const collapsed = new SvelteSet<GitOpsListGroup>();
	const entries = $derived(buildGitOpsListEntries(selections, collapsed));
	const columns = $derived(gitOpsListColumns(selections[0]));

	function toggleGroup(group: GitOpsListGroup) {
		if (!collapsed.delete(group)) collapsed.add(group);
	}

	function healthLabel(selection: GitOpsSelection): string {
		const assessment = selection.item.healthAssessment;
		if (!assessment) return "Unavailable";
		const label = healthStateLabel(assessment.state);
		return assessment.completeness === "partial" ? `${label} (partial)` : label;
	}

	function healthSource(selection: GitOpsSelection): string {
		const assessment = selection.item.healthAssessment;
		return assessment ? healthSourceSummary(assessment) : "Backend contract missing";
	}

	function sourceIconClass(sourceMode: GitOpsSourceMode | null): string {
		if (sourceMode === "git") return "text-sky-300";
		if (sourceMode === "helm") return "text-violet-300";
		if (sourceMode === "multi") return "text-amber-300";
		if (sourceMode === "plugin") return "text-emerald-300";
		return "text-muted-foreground";
	}

	function destination(selection: GitOpsSelection): string {
		if (selection.type === "flux" || selection.type === "argoProject") {
			return selection.item.namespace ?? "-";
		}
		return selection.item.destinationNamespace ?? selection.item.destinationServer ?? "-";
	}

	function revisionCopyValue(selection: GitOpsSelection): string {
		const values = gitOpsSelectionRevisionTooltipRows(selection).flatMap((row) =>
			row.fields.map((field) => `${row.name} ${field.label}: ${field.value}`),
		);
		return values.length > 0
			? values.join("\n")
			: (gitOpsSelectionRevisionLabel(selection) ?? "Revision unavailable");
	}
</script>

<div class="@container overflow-x-auto rounded-lg border">
	<div class={`${columnsClass} border-b bg-muted/30 py-2 text-[0.65rem] font-semibold uppercase tracking-wide text-muted-foreground`}>
		<span>Name</span>
		<span>Health</span>
		<span>{columns.sync}</span>
		<span>Source(s)</span>
		<span>Revision</span>
		<span>{columns.destination}</span>
		<span>Age</span>
		<span class="sticky right-0 flex self-stretch items-center justify-end bg-muted text-right">Actions</span>
	</div>
	<VirtualCollection items={entries} itemKey={gitOpsListEntryKey} {selectedKey} estimateSize={37} {minWidth}>
	{#snippet children(entry)}
		{#if entry.kind === "group"}
			<button
				type="button"
				class="flex w-full min-w-[53.75rem] cursor-pointer items-center gap-2 border-b bg-muted/30 px-3 py-1.5 text-left text-xs font-semibold focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50"
				class:border-b-0={entry.last}
				aria-expanded={!entry.collapsed}
				data-gitops-group={entry.group}
				data-gitops-group-position={entry.position}
				onclick={() => toggleGroup(entry.group)}
			>
				<ChevronDown
					class={`size-3.5 shrink-0 text-muted-foreground transition-transform motion-reduce:transition-none ${entry.collapsed ? "-rotate-90" : ""}`}
					aria-hidden="true"
				/>
				<span>{entry.label}</span>
				<span class="rounded bg-muted px-1.5 py-0.5 text-xs font-normal leading-none tabular-nums">{entry.count}</span>
			</button>
		{:else}
			{@const item = entry.selection}
			{@const selected = gitOpsSelectionKey(item) === selectedKey}
			{@const state = gitOpsListState(item)}
			{@const health = healthPresentation[state]}
			{@const HealthIcon = health.icon}
			{@const request = trackedRequest(item)}
			{@const badges = gitOpsCardBadges(item, request)}
			{@const sync = gitOpsListSync(item, request)}
			{@const sourceMode = gitOpsSelectionSourceMode(item)}
			{@const SourceIcon = gitOpsSourceIcon(sourceMode)}
			{@const sourceLabel = gitOpsSelectionSourceLabel(sourceMode, argoSourceCount(item))}
			{@const sourceTooltipText = gitOpsSelectionSourceTooltip(item)}
			{@const sourceLine = gitOpsSelectionSourceLine(item)}
			<div
				class={selected
					? "group relative min-w-[53.75rem] border-b bg-accent/40 text-sm transition-colors hover:bg-accent/50 motion-reduce:transition-none"
					: "group relative min-w-[53.75rem] border-b bg-surface-1 text-sm transition-colors hover:bg-accent/30 motion-reduce:transition-none"}
				class:border-b-0={entry.last}
			>
				{#if state === "degraded" || state === "needsAttention"}
					<div class={`absolute inset-y-0 left-0 w-1 ${gitOpsCardTone(item)}`}></div>
				{/if}
				<div class={`${columnsClass} h-9`}>
					<div class="flex min-w-0 items-center gap-1.5 pl-1">
						<!-- The name keeps its width and the subtitle gives way first. A shared shrink would
						     cut the name with an ellipsis for a sub-pixel overflow. -->
						<CopyableText
							value={item.item.name}
							label="GitOps resource name"
							class="max-w-full shrink-0"
							textClass="font-medium underline-offset-4 hover:underline"
							onActivate={() => onOpen(item)}
							actionLabel={gitOpsCardActionLabel(item)}
							active={selected}
						/>
						<span class="min-w-0 truncate text-xs text-muted-foreground">{gitOpsCardSubtitle(item)}</span>
					</div>
					<LazyTooltip
						class="flex min-w-0 items-center gap-1.5 rounded-sm text-left text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
						onkeydown={stopTooltipEvent}
						side="top" align="start" sideOffset={8} contentClass={healthTooltipClass}>
						{#snippet content()}
							<div class="grid gap-3 text-xs">
								<div>
									<p class="text-sm font-medium">{healthLabel(item)}</p>
									<p class="mt-1 text-muted-foreground">{healthSource(item)}</p>
								</div>
								{#if badges.length > 0}
									<div class="flex flex-wrap gap-2 border-t border-border pt-3">
										{#each badges as [label, value]}
											<Badge variant="outline" class="whitespace-normal break-words">{label}: {formatStatusLabel(value)}</Badge>
										{/each}
									</div>
								{/if}
							</div>
						{/snippet}
						<HealthIcon class={`size-3.5 shrink-0 ${health.iconClass}`} aria-hidden="true" />
						<span class={`truncate ${health.textClass}`}>{healthLabel(item)}</span>
					</LazyTooltip>
					<span class={`truncate text-xs ${syncToneClass[sync.tone]}`}>{sync.label}</span>
					{#if SourceIcon || sourceLine}
						<LazyTooltip
							class="flex min-w-0 items-center gap-2 text-left text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
							aria-label={`${sourceTooltipText}. Activate to copy source details.`}
							onclick={(event: MouseEvent) => onCopy(event, sourceTooltipText, "source details")}
							onkeydown={stopTooltipEvent}
							side="top" align="start" sideOffset={8} contentClass={tooltipClass}>
							{#snippet content()}
								{@render sourceTooltip(gitOpsSelectionSourceTooltipTitle(item), gitOpsSelectionSourceTooltipGroups(item), sourceLine ?? sourceLabel)}
							{/snippet}
							{#if SourceIcon}
								<SourceIcon class={`size-3.5 shrink-0 ${sourceIconClass(sourceMode)}`} aria-hidden="true" />
							{/if}
							<span class="min-w-0 flex-1 truncate">{sourceLine ?? "-"}</span>
						</LazyTooltip>
					{:else}
						<span class="text-muted-foreground">-</span>
					{/if}
					<LazyTooltip
						class="truncate text-left underline-offset-4 hover:underline"
						aria-label={`Revision: ${revisionCopyValue(item)}. Activate to copy.`}
						onclick={(event: MouseEvent) => onCopy(event, revisionCopyValue(item), "revision")}
						onkeydown={stopTooltipEvent}
						side="top" align="start" sideOffset={8} contentClass="max-w-[34rem] break-all">
						{#snippet content()}
							{revisionCopyValue(item)}
						{/snippet}
						{gitOpsSelectionRevisionLabel(item)}
					</LazyTooltip>
					<span class="truncate text-muted-foreground">{destination(item)}</span>
					<span class="tabular-nums text-muted-foreground">{item.item.age ?? "-"}</span>
					<div
						class={selected
							? "sticky right-0 flex h-full items-center justify-end gap-1 bg-accent group-hover:bg-accent"
							: "sticky right-0 flex h-full items-center justify-end gap-1 bg-surface-1 group-hover:bg-accent"}
					>
						<Button
							type="button"
							variant="ghost"
							size="sm"
							aria-label={gitOpsDetailsActionLabel(item)}
							data-details-key={gitOpsDetailsActionKey(item)}
							onclick={(event: MouseEvent) => onDetails(event, item)}
							onkeydown={stopTooltipEvent}
						>
							Details
						</Button>
						{#if gitOpsSelectionPrimaryAction(item) === "openResources"}
							<Button
								type="button"
								variant="outline"
								size="sm"
								class="hidden @min-[68rem]:inline-flex"
								onclick={(event: MouseEvent) => onResources(event, item)}
								onkeydown={stopTooltipEvent}
							>
								Resources
							</Button>
						{/if}
					</div>
				</div>
				{#if entry.expanded}
					{@const reasons = item.item.healthAssessment?.reasons ?? []}
					<div class="flex min-w-0 flex-wrap items-center gap-1.5 pb-2 pl-4 pr-3 text-xs text-muted-foreground">
						{#if reasons.length > 0}
							<span class="mr-1">{reasons.join(" · ")}</span>
						{/if}
						{#each badges as [label, value]}
							<Badge variant="outline" class={statusClass(value)}>{label}: {formatStatusLabel(value)}</Badge>
						{/each}
					</div>
				{/if}
			</div>
		{/if}
	{/snippet}
	</VirtualCollection>
</div>
