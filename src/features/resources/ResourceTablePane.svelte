<script lang="ts">
	import { ArrowDown, ArrowUp, ChevronDown, ChevronRight, ChevronsUpDown, PanelRightClose, PanelRightOpen, Pin, Table2 } from "lucide-svelte";
	import { Badge, Button, Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/svelte";
	import { getResourceGroupVisual, getResourceKindVisual } from "@/app/svelte/resourceVisuals";
	import CopyableText from "@/components/CopyableText.svelte";
	import TimestampText from "@/components/TimestampText.svelte";
	import { STATUS_BADGE_STYLES } from "@/components/status-badge-styles";
	import type { ChipVariant } from "@/features/argo/status";
	import { gitOpsOwnership } from "@/lib/gitops-ownership-evidence";
	import { findResourceIndex, resourceKey } from "@/lib/resource-identity";
	import { formatCpuMillicores, formatMemoryBytes, resourceMetricFor, type ResourceMetricIndex } from "@/lib/resource-metrics";
	import type { ResourceSummary } from "@/lib/types";
	import { cnfast } from "@/lib/utils";
	import { EMPTY_PAGE_CLASS, ROW_CLASS, SELECTED_ROW_CLASS, STICKY_APP_GROUP_TOP, STICKY_TYPE_GROUP_TOP, TABLE_CLASS } from "./constants";
	import { resourceReadyChip } from "./helpers";
	import { CPU_USAGE_DESCRIPTION, MEMORY_USAGE_DESCRIPTION, READINESS_NOT_REPORTED, RESTART_COUNT_NOT_REPORTED } from "./operational-data";
	import type { ResourceSortColumn, ResourceTableModel, ResourceTablePage } from "./resourceTableTypes";
	import ResourceTableStatus from "./ResourceTableStatus.svelte";

	let {
		tableModel, tablePanelOpen = $bindable(), tableHeadingElement = $bindable(),
		pageIndex = $bindable(), sortColumn = $bindable(), sortDesc = $bindable(),
		collapsedGroups = $bindable(), selectedResource, metricsIndex, contexts, title,
		pinnedResourceKeys, updatesCurrent, selectResource, onResourcePinToggle,
	}: {
		tableModel: ResourceTablePage & Pick<ResourceTableModel, "entries">;
		tablePanelOpen: boolean;
		tableHeadingElement: HTMLHeadingElement | undefined;
		pageIndex: number;
		sortColumn: ResourceSortColumn;
		sortDesc: boolean;
		collapsedGroups: Set<string>;
		selectedResource: ResourceSummary | null;
		metricsIndex: ResourceMetricIndex;
		contexts: readonly string[];
		title: string;
		pinnedResourceKeys: readonly string[];
		updatesCurrent: boolean;
		selectResource: (resource: ResourceSummary) => void;
		onResourcePinToggle: (resource: ResourceSummary) => void;
	} = $props();

	const EMPTY_CELL = "—";
	const multiContext = $derived(tableModel.multiContext);
	const pinnedResourceKeySet = $derived(new Set(pinnedResourceKeys));
	let tableViewportElement = $state<HTMLDivElement | null>(null);
	let appliedSelectionScrollKey = $state("");
	const pageMetrics = $derived(tableModel.pageRows.map((row) => resourceMetricFor(metricsIndex, row, row.metrics)));
	const cpuVisible = $derived(pageMetrics.some((metric) => metric?.cpuMillicores !== undefined));
	const memoryVisible = $derived(pageMetrics.some((metric) => metric?.memoryBytes !== undefined));
	const tableVisibleColumnCount = $derived(
		6 + Number(multiContext) +
			Number(tableModel.columnVisibility.ready) +
			Number(tableModel.columnVisibility.restarts) +
			Number(cpuVisible) +
			Number(memoryVisible) +
			Number(tableModel.columnVisibility.gitOps),
	);
	const tableMinWidth = $derived(
		250 +
			130 + (multiContext ? 130 : 0) +
			105 +
			100 +
			(tableModel.columnVisibility.ready ? 70 : 0) +
			(tableModel.columnVisibility.restarts ? 75 : 0) +
			(cpuVisible ? 65 : 0) +
			(memoryVisible ? 85 : 0) +
			(tableModel.columnVisibility.gitOps ? 110 : 0) +
			65 +
			40,
	);
	const tableStickyMeasureKey = $derived(
		`${tablePanelOpen}:${tableModel.entries.length}:${tableVisibleColumnCount}`,
	);
	const selectedResourceKey = $derived(
		selectedResource ? resourceKey(selectedResource) : "",
	);
	const selectedPageRow = $derived(
		selectedResource
			? tableModel.pageRows[findResourceIndex(tableModel.pageRows, selectedResource, (row) => row)]
			: undefined,
	);
	$effect(() => {
		const measureKey = tableStickyMeasureKey;
		const viewport = tableViewportElement;
		if (!tablePanelOpen || !viewport || !measureKey) return;
		const measure = () => measureTableStickyOffsets(viewport);
		const frame = window.requestAnimationFrame(measure);
		const resizeObserver = "ResizeObserver" in globalThis
			? new ResizeObserver(measure)
			: null;
		resizeObserver?.observe(viewport);
		return () => {
			window.cancelAnimationFrame(frame);
			resizeObserver?.disconnect();
		};
	});

	$effect(() => {
		const viewport = tableViewportElement;
		const scrollKey = selectedResourceKey;
		if (!scrollKey) {
			appliedSelectionScrollKey = "";
			return;
		}
		if (appliedSelectionScrollKey === scrollKey) return;
		const entries = tableModel.entries;
		if (!tablePanelOpen || !viewport || entries.length === 0) {
			return;
		}
		appliedSelectionScrollKey = scrollKey;
		let secondFrame: number | null = null;
		const firstFrame = window.requestAnimationFrame(() => {
			scrollSelectedTableRowIntoView(viewport);
			secondFrame = window.requestAnimationFrame(() => scrollSelectedTableRowIntoView(viewport));
		});
		return () => {
			window.cancelAnimationFrame(firstFrame);
			if (secondFrame !== null) window.cancelAnimationFrame(secondFrame);
		};
	});

	function toggleSort(column: ResourceSortColumn) {
		if (sortColumn === column) {
			sortDesc = !sortDesc;
		} else {
			sortColumn = column;
			sortDesc = false;
		}
	}

	function toggleGroup(key: string) {
		const next = new Set(collapsedGroups);
		if (next.has(key)) next.delete(key);
		else next.add(key);
		collapsedGroups = next;
	}

	function scrollSelectedTableRowIntoView(viewport: HTMLDivElement) {
		const selectedRow = viewport.querySelector<HTMLElement>(
			"tr[data-resource-selected='true']",
		);
		selectedRow?.scrollIntoView({ block: "center", inline: "nearest" });
	}

	function measureTableStickyOffsets(viewport: HTMLDivElement) {
		const headerRow = viewport.querySelector("thead tr");
		const appGroupCell = viewport.querySelector('[data-sticky="app-group"]');
		const headerHeight = headerRow ? Math.round(headerRow.getBoundingClientRect().height) : 0;
		const appGroupHeight = appGroupCell
			? Math.round(appGroupCell.getBoundingClientRect().height)
			: 0;
		viewport.style.setProperty("--sticky-app-top", `${Math.max(headerHeight - 1, 0)}px`);
		viewport.style.setProperty(
			"--sticky-type-top",
			`${Math.max(headerHeight + appGroupHeight - 2, 0)}px`,
		);
	}

	function statusBadgeVariant(tone: ChipVariant) {
		return STATUS_BADGE_STYLES[tone].variant;
	}

	function statusBadgeClass(tone: ChipVariant) {
		return `rounded-full px-2 py-0 text-[0.6875rem] shadow-none ${STATUS_BADGE_STYLES[tone].className}`;
	}
</script>

{#if tablePanelOpen}
	<aside
		class="flex h-full min-h-[400px] min-w-0 flex-col overflow-hidden rounded-lg border bg-surface-1 shadow-sm"
	>
		<div class="flex items-start justify-between gap-2 border-b px-3 py-2">
			<div class="min-w-0">
				<div class="flex min-w-0 items-center gap-2">
					<Table2 class="size-4 shrink-0 text-muted-foreground" />
					<h2 class="truncate text-sm font-semibold" bind:this={tableHeadingElement} tabindex="-1">Resource Table</h2>
				</div>
				<p class="mt-0.5 text-xs text-muted-foreground">
					{tableModel.totalRows} resources{multiContext ? ` · ${contexts.length} contexts` : ""} · page {tableModel.safePageIndex + 1} of {tableModel.pageCount}
				</p>
			</div>
			<Button
				type="button"
				variant="ghost"
				size="icon"
				class="size-7"
				onclick={() => (tablePanelOpen = false)}
				aria-label="Collapse resource table"
			>
				<PanelRightClose />
			</Button>
		</div>
		<div
			bind:this={tableViewportElement}
			class="scrollbar-classic h-full min-h-0 flex-1 overflow-auto [&_[data-slot=table-container]]:overflow-visible"
		>
			<Table aria-label={`${title} resource table`} class={TABLE_CLASS} style={`min-width: ${tableMinWidth}px;`}>
				<colgroup>
					<col />
					<col style="width: 130px;" />
					{#if multiContext}<col style="width: 130px;" />{/if}
					<col style="width: 105px;" />
					<col style="width: 100px;" />
					{#if tableModel.columnVisibility.ready}<col style="width: 70px;" />{/if}
					{#if tableModel.columnVisibility.restarts}<col style="width: 75px;" />{/if}
					{#if cpuVisible}<col style="width: 65px;" />{/if}
					{#if memoryVisible}<col style="width: 85px;" />{/if}
					{#if tableModel.columnVisibility.gitOps}<col style="width: 110px;" />{/if}
					<col style="width: 65px;" />
					<col style="width: 40px;" />
				</colgroup>
				<TableHeader>
					<TableRow>
						<TableHead>{@render SortButton("name", "Name")}</TableHead>
						<TableHead>{@render SortButton("status", "Status")}</TableHead>
						{#if multiContext}<TableHead>{@render SortButton("context", "Context")}</TableHead>{/if}
						<TableHead>{@render SortButton("namespace", "Namespace")}</TableHead>
						<TableHead>{@render SortButton("kind", "Kind")}</TableHead>
						{#if tableModel.columnVisibility.ready}<TableHead>{@render SortButton("ready", "Ready")}</TableHead>{/if}
						{#if tableModel.columnVisibility.restarts}<TableHead>{@render SortButton("restarts", "Restarts")}</TableHead>{/if}
						{#if cpuVisible}<TableHead>{@render SortButton("cpu", "CPU", CPU_USAGE_DESCRIPTION)}</TableHead>{/if}
						{#if memoryVisible}<TableHead>{@render SortButton("memory", "Memory", MEMORY_USAGE_DESCRIPTION)}</TableHead>{/if}
						{#if tableModel.columnVisibility.gitOps}<TableHead>Owner</TableHead>{/if}
						<TableHead>{@render SortButton("age", "Age")}</TableHead>
						<TableHead><span class="sr-only">Pin</span></TableHead>
					</TableRow>
				</TableHeader>
				<TableBody>
					{#if tableModel.entries.length === 0}
						<TableRow>
							<TableCell colspan={tableVisibleColumnCount} class={EMPTY_PAGE_CLASS}>
								No resources match current scope and filters.
							</TableCell>
						</TableRow>
					{:else}
						{#each tableModel.entries as entry (entry.key)}
							{#if entry.type === "group"}
								{@const visual = getResourceGroupVisual(entry.label)}
								{@const GroupIcon = visual.icon}
								<TableRow class="text-xs font-bold text-primary hover:bg-transparent">
									<TableCell
										colspan={tableVisibleColumnCount}
										data-sticky="app-group"
										class={cnfast("sticky z-20 !p-0 bg-background", STICKY_APP_GROUP_TOP)}
									>
				<Button
					type="button"
					variant="ghost"
					class="h-auto w-full justify-start rounded-none border-0 bg-muted/50 px-3 py-2 text-left text-inherit hover:bg-muted/60"
					onclick={() => toggleGroup(entry.key)}
					aria-expanded={!entry.collapsed}
				>
											{#if entry.collapsed}
												<ChevronRight class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
											{:else}
												<ChevronDown class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
											{/if}
											<GroupIcon class={cnfast("size-3.5 shrink-0", visual.className)} aria-hidden="true" />
											<span class="text-muted-foreground">{entry.label}</span>
											<small class="text-[0.6875rem] font-medium text-muted-foreground">
						{entry.count} resources on this page
					</small>
				</Button>
									</TableCell>
								</TableRow>
							{:else if entry.type === "type"}
								{@const visual = getResourceKindVisual(entry.kind)}
								{@const TypeIcon = visual.icon}
								<TableRow class="text-[0.72rem] font-bold uppercase text-foreground hover:bg-transparent">
									<TableCell
										colspan={tableVisibleColumnCount}
										class={cnfast("sticky z-10 !p-0 bg-card", STICKY_TYPE_GROUP_TOP)}
									>
				<Button
					type="button"
					variant="ghost"
					class={cnfast(
						"h-auto w-full justify-start rounded-none border-0 bg-card py-1.5 pr-3 text-left text-[0.6875rem] text-inherit hover:bg-muted/40",
						tableModel.groupedByGitOps ? "pl-6" : "pl-3",
					)}
					onclick={() => toggleGroup(entry.key)}
											aria-expanded={!entry.collapsed}
										>
											{#if entry.collapsed}
												<ChevronRight class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
											{:else}
												<ChevronDown class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
											{/if}
											<TypeIcon class={cnfast("size-3.5 shrink-0", visual.className)} aria-hidden="true" />
											<span>{entry.label}</span>
											<small class="text-[0.625rem] font-medium normal-case text-muted-foreground">
						{entry.count} on this page
					</small>
				</Button>
									</TableCell>
								</TableRow>
							{:else}
								{@const row = entry.resource}
								{@const metric = resourceMetricFor(metricsIndex, row, row.metrics)}
								{@const rowSelected = row === selectedPageRow}
								<TableRow
									data-resource-selected={rowSelected ? "true" : undefined}
									class={cnfast(ROW_CLASS, "cursor-pointer", rowSelected && SELECTED_ROW_CLASS)}
									onclick={(event: MouseEvent) => {
										if (event.target instanceof Element && event.target.closest("button")) return;
										selectResource(row);
									}}
								>
									<TableCell class="font-medium">
										<CopyableText
											value={row.name}
											label="resource name"
											onActivate={() => selectResource(row)}
											actionLabel={`Open resource ${row.name}`}
											active={rowSelected}
										/>
									</TableCell>
									<TableCell>
									<ResourceTableStatus resource={row} active={updatesCurrent} />
									</TableCell>
								{#if multiContext}<TableCell><span class="block min-w-0 truncate" title={row.cluster}>{row.cluster}</span></TableCell>{/if}
								<TableCell>
									<span class="block min-w-0 truncate" title={row.namespace ?? EMPTY_CELL}>{row.namespace ?? EMPTY_CELL}</span>
								</TableCell>
								<TableCell>
									<span class="block min-w-0 truncate" title={row.kind}>{row.kind}</span>
								</TableCell>
									{#if tableModel.columnVisibility.ready}
										<TableCell>
											{@const ready = resourceReadyChip(row)}
											{#if ready}
												<Badge variant={statusBadgeVariant(ready.tone)} class={statusBadgeClass(ready.tone)}>
													{ready.value}
												</Badge>
											{:else}
												<span title={READINESS_NOT_REPORTED} aria-label={READINESS_NOT_REPORTED}>{row.ready ?? EMPTY_CELL}</span>
											{/if}
										</TableCell>
									{/if}
									{#if tableModel.columnVisibility.restarts}
										<TableCell>
											{#if row.restarts === undefined || row.restarts === null}
												<span class="flex justify-center" title={RESTART_COUNT_NOT_REPORTED} aria-label={RESTART_COUNT_NOT_REPORTED}>{EMPTY_CELL}</span>
											{:else if row.restarts === 0}
												<span class="flex justify-center">{row.restarts}</span>
											{:else}
												{@const restartTone = row.restarts > 5 ? "error" : "warning"}
												<span class="flex justify-center">
													<Badge variant={statusBadgeVariant(restartTone)} class={statusBadgeClass(restartTone)}>
														{row.restarts}
													</Badge>
												</span>
											{/if}
										</TableCell>
									{/if}
									{#if cpuVisible}
											<TableCell title={CPU_USAGE_DESCRIPTION} aria-label={`CPU usage ${formatCpuMillicores(metric?.cpuMillicores)}`}>{formatCpuMillicores(metric?.cpuMillicores)}</TableCell>
									{/if}
									{#if memoryVisible}
											<TableCell title={MEMORY_USAGE_DESCRIPTION} aria-label={`Memory usage ${formatMemoryBytes(metric?.memoryBytes)}`}>{formatMemoryBytes(metric?.memoryBytes)}</TableCell>
									{/if}
									{#if tableModel.columnVisibility.gitOps}
										<TableCell>
											{@const owner = gitOpsOwnership(row)?.ownerName ?? row.helmRelease ?? EMPTY_CELL}
											<span class="block min-w-0 truncate" title={owner}>{owner}</span>
										</TableCell>
									{/if}
									<TableCell><TimestampText value={row.createdAt} relative={row.age} /></TableCell>
									<TableCell>
										<Button
											type="button"
											variant="ghost"
											size="icon"
											class="size-7"
											aria-label={`${pinnedResourceKeySet.has(resourceKey(row)) ? "Unpin" : "Pin"} ${row.kind} ${row.name}`}
											onclick={(event: MouseEvent) => {
												event.stopPropagation();
												onResourcePinToggle(row);
											}}
											onkeydown={(event: KeyboardEvent) => event.stopPropagation()}
										>
											<Pin
												class={pinnedResourceKeySet.has(resourceKey(row)) ? "fill-current" : ""}
											/>
										</Button>
									</TableCell>
								</TableRow>
							{/if}
						{/each}
					{/if}
				</TableBody>
			</Table>
		</div>
		<div class="grid grid-cols-[auto_1fr_auto] items-center gap-2 border-t px-3 py-2 text-xs text-muted-foreground">
			<Button
				variant="outline"
				size="sm"
				disabled={tableModel.safePageIndex === 0}
				onclick={() => (pageIndex = Math.max(0, tableModel.safePageIndex - 1))}
			>
				Previous
			</Button>
			<div class="grid grid-cols-2 text-center">
				<span>{tableModel.totalRows} total rows</span>
				<span>Page {tableModel.safePageIndex + 1} of {tableModel.pageCount}</span>
			</div>
			<Button
				variant="outline"
				size="sm"
				disabled={tableModel.safePageIndex >= tableModel.pageCount - 1}
				onclick={() =>
					(pageIndex = Math.min(
						tableModel.pageCount - 1,
						tableModel.safePageIndex + 1,
					))}
			>
				Next
			</Button>
		</div>
	</aside>
{:else}
	<aside
		class="flex h-full min-h-[400px] w-12 shrink-0 flex-col items-center overflow-hidden rounded-lg border bg-surface-1 shadow-sm"
	>
		<div class="flex w-full justify-center border-b p-2">
			<Button
				type="button"
				variant="ghost"
				size="icon"
				class="size-7"
				onclick={() => (tablePanelOpen = true)}
				aria-label="Show resource table"
			>
				<PanelRightOpen />
			</Button>
		</div>
		<Button
			type="button"
			variant="ghost"
			class="h-auto min-h-0 w-full flex-1 flex-col gap-2 rounded-none border-0 px-2 py-3 text-muted-foreground hover:bg-transparent hover:text-foreground"
			onclick={() => (tablePanelOpen = true)}
			aria-label="Show resource table"
		>
			<Table2 class="size-4 shrink-0" />
			<span class="[writing-mode:vertical-rl] text-xs font-semibold">Table</span>
		</Button>
	</aside>
{/if}

{#snippet SortButton(column: ResourceSortColumn, label: string, description?: string)}
	<Button
		type="button"
		variant="ghost"
		size="xs"
		class="h-auto gap-1 border-0 bg-transparent p-0 text-left text-inherit hover:bg-transparent"
		onclick={() => toggleSort(column)}
		aria-label={`Sort by ${label}${description ? `. ${description}` : ""}`}
		title={description}
	>
		{label}
		{#if sortColumn === column && sortDesc}<ArrowDown class="size-3" aria-hidden="true" />{:else if sortColumn === column}<ArrowUp class="size-3" aria-hidden="true" />{:else}<ChevronsUpDown class="size-3" aria-hidden="true" />{/if}
	</Button>
{/snippet}
