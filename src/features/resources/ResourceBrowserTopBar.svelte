<script lang="ts">
	import { ChevronDown, Search, X } from "lucide-svelte";
	import {
		Badge,
		Button,
		Checkbox,
		InputGroup,
		InputGroupAddon,
		InputGroupInput,
		InputGroupText,
		Label,
		Popover,
		PopoverContent,
		PopoverTrigger,
		ScrollArea,
		Select,
		SelectContent,
		SelectGroup,
		SelectItem,
		SelectTrigger,
		SelectValue,
	} from "@/components/ui/svelte";
	import type { NamespaceSummary, ResourceKindSelection } from "@/lib/types";
	import type { GitOpsFilterOption, HealthFilter, HealthSummary } from "./helpers";
	import { kindSelectionKey, kindSelectionLabel } from "./resourceBrowserModel";

	let {
		selectedNamespaces,
		selectedKinds,
		namespaceOptions,
		kindOptions,
		selectedNamespaceSet,
		selectedKindSet,
		healthSummary,
		healthFilter,
		search = $bindable(""),
		gitOpsFilter,
		gitOpsFilters,
		metricsMessage,
		customResourcesStatus,
		realtimeStatus,
		realtimeMessage,
		onAllNamespacesSelect,
		onNamespaceToggle,
		onKindToggle,
		onHealthSelect,
		onGitOpsFilterChange,
		onSearchInput,
		onClearFilters,
	}: {
		selectedNamespaces: string[];
		selectedKinds: ResourceKindSelection[];
		namespaceOptions: NamespaceSummary[];
		kindOptions: ResourceKindSelection[];
		selectedNamespaceSet: Set<string>;
		selectedKindSet: Set<string>;
		healthSummary: HealthSummary;
		healthFilter: HealthFilter;
		search?: string;
		gitOpsFilter: string;
		gitOpsFilters: GitOpsFilterOption[];
		metricsMessage: string | null;
		customResourcesStatus?: string | null;
		realtimeStatus: string;
		realtimeMessage: string;
		onAllNamespacesSelect: () => void;
		onNamespaceToggle: (namespace: string, checked: boolean) => void;
		onKindToggle: (kind: ResourceKindSelection, checked: boolean) => void;
		onHealthSelect: (filter: HealthFilter) => void;
		onGitOpsFilterChange: (value: string) => void;
		onSearchInput: () => void;
		onClearFilters: () => void;
	} = $props();

	let namespacePopoverOpen = $state(false);
	let kindPopoverOpen = $state(false);

	const hasFilters = $derived(Boolean(search || gitOpsFilter || healthFilter !== "all"));
	const namespaceLabel = $derived(
		selectedNamespaces.length === 0
			? "All"
			: selectedNamespaces.length <= 2
				? selectedNamespaces.join(", ")
				: `${selectedNamespaces.slice(0, 2).join(", ")} +${selectedNamespaces.length - 2}`,
	);
	const kindsLabel = $derived(
		selectedKinds.length <= 3
			? selectedKinds.map(kindSelectionLabel).join(", ")
			: `${selectedKinds.slice(0, 3).map(kindSelectionLabel).join(", ")} +${selectedKinds.length - 3}`,
	);

	function healthButtonClass(active: boolean) {
		return [
			"h-auto min-h-12 w-full flex-col items-start justify-center gap-0.5 bg-background/30 px-2.5 py-1.5 text-left font-normal hover:bg-background/50",
			active ? "border-primary/50 bg-primary/10" : "",
		].join(" ");
	}

	function countClass(value: number, tone?: "success" | "warning" | "danger" | "info") {
		if (value === 0) return "text-muted-foreground/70";
		if (tone === "success") return "text-emerald-300";
		if (tone === "warning") return "text-amber-300";
		if (tone === "danger") return "text-red-300";
		if (tone === "info") return "text-sky-300";
		return "text-foreground";
	}
</script>

<section class="@container rounded-lg border bg-surface-1 p-2 shadow-sm" aria-label="Resource controls">
	<div class="grid gap-2 @5xl:grid-cols-[auto_minmax(18rem,1fr)_auto] @5xl:items-center">
		<div class="flex min-w-0 flex-wrap items-center gap-2" aria-label="Current resource scope">
			<Popover bind:open={namespacePopoverOpen}>
				<PopoverTrigger
					type="button"
					class="inline-flex h-8 min-w-0 max-w-full cursor-pointer items-center gap-1.5 rounded-md border bg-background/70 px-2.5 text-xs text-muted-foreground hover:bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
					aria-label="Edit namespace scope"
				>
					Namespace
					<strong class="min-w-0 truncate font-semibold text-foreground">{namespaceLabel}</strong>
					<ChevronDown class="size-3.5 shrink-0" />
				</PopoverTrigger>
				<PopoverContent align="start" class="w-72 p-2">
					<div class="mb-2 px-1 text-[0.625rem] font-semibold uppercase text-muted-foreground">
						Namespaces
					</div>
					<Label class="cursor-pointer rounded-md px-2 py-1.5 hover:bg-muted/50">
						<Checkbox
							checked={selectedNamespaces.length === 0}
							onCheckedChange={(checked) => {
								if (checked || selectedNamespaces.length > 0) onAllNamespacesSelect();
							}}
						/>
						All namespaces
					</Label>
					<ScrollArea class="mt-1 h-40 rounded-md border bg-background/30">
						<div class="flex flex-col gap-1 p-1">
							{#each namespaceOptions as namespace (namespace.name)}
								<Label class="cursor-pointer rounded-md px-2 py-1.5 hover:bg-muted/50">
									<Checkbox
										checked={selectedNamespaceSet.has(namespace.name)}
										onCheckedChange={(checked) => onNamespaceToggle(namespace.name, checked)}
									/>
									<span class="truncate">{namespace.name}</span>
								</Label>
							{:else}
								<div class="px-2 py-1.5 text-xs text-muted-foreground">No namespaces found</div>
							{/each}
						</div>
					</ScrollArea>
				</PopoverContent>
			</Popover>

			<Popover bind:open={kindPopoverOpen}>
				<PopoverTrigger
					type="button"
					class="inline-flex h-8 min-w-0 max-w-full cursor-pointer items-center gap-1.5 rounded-md border bg-background/70 px-2.5 text-xs text-muted-foreground hover:bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
					aria-label="Edit resource kinds"
				>
					Kinds
					<strong class="min-w-0 truncate font-semibold text-foreground">{kindsLabel}</strong>
					<ChevronDown class="size-3.5 shrink-0" />
				</PopoverTrigger>
				<PopoverContent align="start" class="w-[min(32rem,calc(100vw-1rem))] p-2">
					<div class="mb-2 px-1 text-[0.625rem] font-semibold uppercase text-muted-foreground">
						Kinds
					</div>
					<ScrollArea class="h-56 rounded-md border bg-background/30">
						<div class="grid gap-1 p-1 sm:grid-cols-2">
							{#each kindOptions as kind (kindSelectionKey(kind))}
								<Label class="cursor-pointer rounded-md px-2 py-1.5 hover:bg-muted/50">
									<Checkbox
										checked={selectedKindSet.has(kindSelectionKey(kind))}
										onCheckedChange={(checked) => onKindToggle(kind, checked)}
									/>
									<span class="truncate">{kindSelectionLabel(kind)}</span>
								</Label>
							{:else}
								<div class="px-2 py-1.5 text-xs text-muted-foreground">No kinds found</div>
							{/each}
						</div>
					</ScrollArea>
				</PopoverContent>
			</Popover>
		</div>

		<InputGroup class="h-8 min-w-0 border bg-background/70">
			<InputGroupAddon align="inline-start">
				<InputGroupText><Search class="size-4" /></InputGroupText>
			</InputGroupAddon>
			<InputGroupInput
				aria-label="Search resources"
				class="h-7 text-xs"
				bind:value={search}
				placeholder="Search by name, namespace, kind, owner, GitOps owner, Helm release..."
				oninput={onSearchInput}
			/>
		</InputGroup>

		<div class="flex min-w-0 flex-wrap items-center gap-2 @5xl:justify-end">
			{#if gitOpsFilters.length > 0}
				<Select
					value={gitOpsFilter || "__all"}
					items={[
						{ value: "__all", label: "All GitOps owners" },
						...gitOpsFilters.map((filter) => ({
							value: filter.key,
							label: filter.label,
						})),
					]}
					onValueChange={onGitOpsFilterChange}
				>
					<SelectTrigger class="h-8 max-w-56 border bg-background/70">
						<SelectValue placeholder="All GitOps owners" />
					</SelectTrigger>
					<SelectContent>
						<SelectGroup>
							<SelectItem value="__all">All GitOps owners</SelectItem>
							{#each gitOpsFilters as filter (filter.key)}
								<SelectItem value={filter.key}>{filter.label}</SelectItem>
							{/each}
						</SelectGroup>
					</SelectContent>
				</Select>
			{/if}
			{#if hasFilters}
				<Button variant="outline" size="sm" class="h-8 bg-background/70" onclick={onClearFilters}>
					<X data-icon="inline-start" />
					Clear
				</Button>
			{/if}
		</div>
	</div>

	<div class="mt-2 grid gap-2 border-t pt-2 @5xl:grid-cols-2 @5xl:items-stretch">
		<div class="grid grid-cols-2 gap-2 @3xl:grid-cols-3 @7xl:grid-cols-6" aria-label="Resource health summary">
			<Button
				type="button"
				variant="outline"
				class={healthButtonClass(healthFilter === "all")}
				aria-pressed={healthFilter === "all"}
				onclick={() => onHealthSelect("all")}
			>
				<span class="text-[0.625rem] font-semibold uppercase text-muted-foreground">Total</span>
				<span class="flex items-baseline gap-1.5">
					<strong class={`tabular-nums ${countClass(healthSummary.total)}`}>{healthSummary.total}</strong>
					<span class="text-[0.625rem] text-muted-foreground">{healthSummary.untracked} unchecked</span>
				</span>
			</Button>
			<Button
				type="button"
				variant="outline"
				class={healthButtonClass(healthFilter === "healthy")}
				aria-pressed={healthFilter === "healthy"}
				onclick={() => onHealthSelect("healthy")}
			>
				<span class="text-[0.625rem] font-semibold uppercase text-muted-foreground">Healthy</span>
				<strong class={`tabular-nums ${countClass(healthSummary.healthy, "success")}`}>
					{healthSummary.healthy}
				</strong>
			</Button>
			<Button
				type="button"
				variant="outline"
				class={healthButtonClass(healthFilter === "unhealthy")}
				aria-pressed={healthFilter === "unhealthy"}
				onclick={() => onHealthSelect("unhealthy")}
			>
				<span class="text-[0.625rem] font-semibold uppercase text-muted-foreground">Unhealthy</span>
				<strong
					class={`tabular-nums ${countClass(healthSummary.attention + healthSummary.degraded, "warning")}`}
				>
					{healthSummary.attention + healthSummary.degraded}
				</strong>
			</Button>
			<Button
				type="button"
				variant="outline"
				class={healthButtonClass(healthFilter === "attention")}
				aria-pressed={healthFilter === "attention"}
				onclick={() => onHealthSelect("attention")}
			>
				<span class="text-[0.625rem] font-semibold uppercase text-muted-foreground">Needs attention</span>
				<strong class={`tabular-nums ${countClass(healthSummary.attention, "warning")}`}>
					{healthSummary.attention}
				</strong>
			</Button>
			<Button
				type="button"
				variant="outline"
				class={healthButtonClass(healthFilter === "degraded")}
				aria-pressed={healthFilter === "degraded"}
				onclick={() => onHealthSelect("degraded")}
			>
				<span class="text-[0.625rem] font-semibold uppercase text-muted-foreground">Degraded</span>
				<strong class={`tabular-nums ${countClass(healthSummary.degraded, "danger")}`}>
					{healthSummary.degraded}
				</strong>
			</Button>
			<Button
				type="button"
				variant="outline"
				class={healthButtonClass(healthFilter === "restarted")}
				aria-pressed={healthFilter === "restarted"}
				onclick={() => onHealthSelect("restarted")}
			>
				<span class="text-[0.625rem] font-semibold uppercase text-muted-foreground">Restarted</span>
				<strong class={`tabular-nums ${countClass(healthSummary.restarted, "info")}`}>
					{healthSummary.restarted}
				</strong>
			</Button>
		</div>

		<div class="grid min-w-0 gap-2 @3xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)] @5xl:grid-cols-[minmax(12rem,1fr)_minmax(12rem,1fr)] @7xl:grid-cols-[minmax(12rem,1fr)_minmax(12rem,1fr)_minmax(12rem,auto)]">
			<div class="rounded-md border bg-background/30 px-3 py-2 text-xs">
				<div class="font-semibold text-foreground">Resource metrics</div>
				<div class="text-muted-foreground">{metricsMessage ?? "metrics available"}</div>
			</div>

			<div class="rounded-md border bg-background/30 px-3 py-2 text-xs">
				<div class="font-semibold text-foreground">Custom Resources</div>
				<div class="text-muted-foreground">{customResourcesStatus ?? "available"}</div>
			</div>

			<div class="flex min-w-0 items-center gap-2 rounded-md border bg-background/30 px-3 py-2 text-xs">
				<Badge variant={realtimeStatus === "error" ? "destructive" : "outline"} class="shrink-0">
					Realtime: {realtimeStatus}
				</Badge>
				<span class="min-w-0 truncate text-muted-foreground">{realtimeMessage}</span>
			</div>
		</div>
	</div>
</section>
