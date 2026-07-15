<script lang="ts">
	import { ExternalLink, Package, Search, X } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Badge,
		Button,
		Card,
		CardContent,
		CardDescription,
		CardFooter,
		CardHeader,
		CardTitle,
		Empty,
		EmptyContent,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Input,
		SegmentedControl,
		Spinner,
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow,
	} from "@/components/ui/svelte";
	import {
		helmReconciliationResourceLabel,
		helmReconciliationStatusLabel,
		helmReleaseKey,
		helmStatusTone,
	} from "@/features/helm/helpers";
	import { settingsStore } from "@/lib/settings-store";
	import type { HelmViewMode } from "@/lib/settings";
	import type {
		HelmManifestResourceSummary,
		HelmReconciliationResource,
		HelmReleaseDetails,
		HelmReleaseReconciliation,
		HelmReleaseSummary,
	} from "@/lib/types";
	import { cn, formatStatusLabel } from "@/lib/utils";
	import SimpleTable from "@/components/SimpleTable.svelte";
	import SurfaceFrame from "@/components/SurfaceFrame.svelte";

	type QueryState<T> = {
		data?: T;
		isPending: boolean;
		isError: boolean;
		error: unknown;
	};

	let { list, details, reconciliation, actions }: {
		list: {
			query: QueryState<HelmReleaseSummary[]>;
			groups: Array<{ namespace: string; releases: HelmReleaseSummary[] }>;
			filtered: HelmReleaseSummary[];
			activeNamespace: string | null;
			selectNamespace: (namespace: string | null) => void;
			search: string;
			setSearch: (search: string) => void;
			selected: HelmReleaseSummary | null;
			selectedKey: string;
			select: (release: HelmReleaseSummary | null) => void;
		};
		details: { query: QueryState<HelmReleaseDetails> };
		reconciliation: {
			query: QueryState<HelmReleaseReconciliation>;
			rows: HelmReconciliationResource[];
			classFor: (status: HelmReconciliationResource["status"]) => string;
			sourceFor: (resource: HelmReconciliationResource) => string;
		};
		actions: {
			openResources: (namespace?: string | string[], initialSearch?: string) => void;
			statusVariant: (status: string | undefined) => "destructive" | "outline";
		};
	} = $props();
	const helmQuery = $derived(list.query);
	const groupedHelmReleases = $derived(list.groups);
	const filteredHelmReleases = $derived(list.filtered);
	const activeHelmNamespace = $derived(list.activeNamespace);
	const helmSearch = $derived(list.search);
	const selectedHelmRelease = $derived(list.selected);
	const selectedHelmReleaseKey = $derived(list.selectedKey);
	const helmDetailsQuery = $derived(details.query);
	const helmReconciliationQuery = $derived(reconciliation.query);
	const helmReconciliationRows = $derived(reconciliation.rows);
	const onOpenResources = $derived(actions.openResources);
	const helmStatusVariant = $derived(actions.statusVariant);
	const helmReconciliationClass = $derived(reconciliation.classFor);
	const helmReconciliationSource = $derived(reconciliation.sourceFor);
	const helmViewOptions: { value: HelmViewMode; label: string }[] = [
		{ value: "cards", label: "Cards" },
		{ value: "list", label: "List" },
	];
	const helmViewMode = $derived($settingsStore.helmViewMode);
	const helmReleases = $derived(helmQuery.data ?? []);
	const deployedReleaseCount = $derived(
		helmReleases.filter((release) => release.status === "deployed").length,
	);
	const pendingReleaseCount = $derived(
		helmReleases.filter((release) => release.status?.startsWith("pending-")).length,
	);
	const failedReleaseCount = $derived(
		helmReleases.filter((release) => release.status === "failed").length,
	);

	function helmStatusClass(status: string | undefined): string {
		const tone = helmStatusTone(status);
		if (tone === "success") return "border-emerald-500/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300";
		if (tone === "warning") return "border-amber-500/40 bg-amber-500/10 text-amber-700 dark:text-amber-300";
		if (tone === "error") return "border-destructive/40 bg-destructive/10 text-destructive";
		return "";
	}

	function helmStatusRailClass(status: string | undefined): string {
		const tone = helmStatusTone(status);
		if (tone === "success") return "bg-emerald-500";
		if (tone === "warning") return "bg-amber-500";
		if (tone === "error") return "bg-destructive";
		return "bg-muted-foreground/45";
	}
</script>

<SurfaceFrame icon={Package} title="Helm" query={helmQuery} errorLabel="Helm releases unavailable" wide>
	<div class="@container flex flex-col gap-4">
		<section
			class="rounded-lg border bg-surface-1 px-3 py-2.5 shadow-sm"
			aria-label="Helm overview"
		>
			<div class="flex flex-wrap items-start justify-between gap-3">
				<div class="min-w-0">
					<h2 class="font-heading text-base font-semibold">Helm</h2>
					{#if helmReleases.length > 0}
						<div class="mt-2">
							<Badge variant="outline" class="border-emerald-500/35 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300">
								Helm storage detected
							</Badge>
						</div>
					{/if}
				</div>
				<div class="text-right">
					<div class="font-semibold tabular-nums">{helmReleases.length}</div>
					<div class="text-xs text-muted-foreground">Helm releases</div>
				</div>
			</div>
			<div class="mt-2.5 grid grid-cols-2 gap-x-4 gap-y-2 @min-[42rem]:grid-cols-5">
				<div class="min-w-0">
					<div class="text-[0.62rem] font-semibold uppercase tracking-wide text-muted-foreground">Releases</div>
					<div class="mt-0.5 font-semibold tabular-nums">{helmReleases.length}</div>
				</div>
				<div class="min-w-0">
					<div class="text-[0.62rem] font-semibold uppercase tracking-wide text-muted-foreground">Namespaces</div>
					<div class="mt-0.5 font-semibold tabular-nums">{groupedHelmReleases.length}</div>
				</div>
				<div class="min-w-0">
					<div class="text-[0.62rem] font-semibold uppercase tracking-wide text-muted-foreground">Deployed</div>
					<div class="mt-0.5 font-semibold text-emerald-700 tabular-nums dark:text-emerald-300">{deployedReleaseCount}</div>
				</div>
				<div class="min-w-0">
					<div class="text-[0.62rem] font-semibold uppercase tracking-wide text-muted-foreground">Pending</div>
					<div class="mt-0.5 font-semibold text-amber-700 tabular-nums dark:text-amber-300">{pendingReleaseCount}</div>
				</div>
				<div class="min-w-0">
					<div class="text-[0.62rem] font-semibold uppercase tracking-wide text-muted-foreground">Failed</div>
					<div class="mt-0.5 font-semibold text-destructive tabular-nums">{failedReleaseCount}</div>
				</div>
			</div>
		</section>

		<div class="grid gap-4 @min-[64rem]:grid-cols-[14rem_minmax(0,1fr)]">
			<aside class="flex gap-5 overflow-x-auto border-b pb-3 @min-[64rem]:block @min-[64rem]:overflow-visible @min-[64rem]:border-b-0 @min-[64rem]:border-r @min-[64rem]:pb-0 @min-[64rem]:pr-4">
				<section class="flex shrink-0 items-center gap-2 @min-[64rem]:block">
					<p class="whitespace-nowrap px-1 text-xs font-semibold uppercase text-muted-foreground">Namespaces</p>
					<div class="flex gap-1 @min-[64rem]:mt-2 @min-[64rem]:grid">
						<button
							type="button"
							class={cn(
								"flex items-center justify-between gap-2 rounded-md px-2.5 py-2 text-left text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring/30",
								activeHelmNamespace === null
									? "bg-accent font-medium"
									: "text-muted-foreground hover:bg-accent/60",
							)}
							aria-current={activeHelmNamespace === null ? "true" : undefined}
							onclick={() => list.selectNamespace(null)}
						>
							<span class="min-w-0 truncate">All releases</span>
							<span class="rounded bg-muted px-1.5 py-0.5 text-xs leading-none tabular-nums">{helmReleases.length}</span>
						</button>
						{#each groupedHelmReleases as group (group.namespace)}
							<button
								type="button"
								class={cn(
									"flex items-center justify-between gap-2 rounded-md px-2.5 py-2 text-left text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring/30",
									activeHelmNamespace === group.namespace
										? "bg-accent font-medium"
										: "text-muted-foreground hover:bg-accent/60",
								)}
								aria-current={activeHelmNamespace === group.namespace ? "true" : undefined}
								onclick={() => list.selectNamespace(group.namespace)}
							>
								<span class="min-w-0 truncate">{group.namespace}</span>
								<span class="rounded bg-muted px-1.5 py-0.5 text-xs leading-none tabular-nums">{group.releases.length}</span>
							</button>
						{/each}
					</div>
				</section>
			</aside>

			<section class="flex min-w-0 flex-col gap-4">
				<div class="flex flex-wrap items-end justify-between gap-3 border-b pb-3">
					<div>
						<h2 class="text-lg font-semibold">Helm Releases</h2>
						<p class="text-sm text-muted-foreground">Chart, namespace, revision, and storage source.</p>
					</div>
					<div class="flex min-w-0 flex-1 flex-wrap items-center justify-end gap-2">
						<div class="relative min-w-56 flex-1 @min-[48rem]:max-w-sm">
							<Search
								class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
								aria-hidden="true"
							/>
							<Input
								class="h-8 pl-8"
								value={helmSearch}
								oninput={(event: Event & { currentTarget: HTMLInputElement }) =>
									list.setSearch(event.currentTarget.value)}
								placeholder="Search releases..."
								aria-label="Search Helm releases"
							/>
						</div>
						{#if helmSearch}
							<Button type="button" variant="outline" size="sm" onclick={() => list.setSearch("")}>
								<X data-icon="inline-start" />
								Clear
							</Button>
						{/if}
						<span class="text-xs text-muted-foreground tabular-nums">
							{filteredHelmReleases.length} item{filteredHelmReleases.length === 1 ? "" : "s"}
						</span>
						<SegmentedControl
							value={helmViewMode}
							options={helmViewOptions}
							onChange={$settingsStore.setHelmViewMode}
							ariaLabel="Helm view mode"
						/>
					</div>
				</div>

				{#if filteredHelmReleases.length === 0}
					<Empty>
						<EmptyHeader>
							<EmptyTitle>{helmReleases.length === 0 ? "No Helm releases" : "No matching releases"}</EmptyTitle>
							<EmptyDescription>
								{helmReleases.length === 0
									? "No Helm release storage objects were found in this cluster."
									: "Clear the search or choose another namespace."}
							</EmptyDescription>
						</EmptyHeader>
						{#if helmReleases.length > 0}
							<EmptyContent>
								<Button
									type="button"
									variant="outline"
									size="sm"
									onclick={() => {
										list.setSearch("");
										list.selectNamespace(null);
									}}
								>
									Clear filters
								</Button>
							</EmptyContent>
						{/if}
					</Empty>
				{:else if helmViewMode === "cards"}
					<div class="grid grid-cols-1 gap-3 @min-[35rem]:grid-cols-2 @min-[64rem]:grid-cols-[repeat(auto-fit,minmax(18.75rem,26.25rem))]">
						{#each filteredHelmReleases as release (helmReleaseKey(release))}
							<Card
								size="sm"
								elevation="raised"
								class={cn(
									"relative h-[18.5rem] overflow-hidden p-0",
									helmReleaseKey(release) === selectedHelmReleaseKey && "ring-2 ring-ring/40",
								)}
							>
								<div class={cn("absolute inset-y-0 left-0 w-1", helmStatusRailClass(release.status))}></div>
								<CardHeader class="flex flex-row items-start justify-between gap-3 pb-0 pl-4">
									<div class="min-w-0">
										<CardTitle class="truncate">{release.name}</CardTitle>
										<CardDescription class="truncate">{release.namespace}</CardDescription>
									</div>
									<div class="grid size-7 shrink-0 place-items-center rounded-md border text-muted-foreground">
										<Package class="size-3.5" aria-hidden="true" />
									</div>
								</CardHeader>
								<CardContent class="flex min-h-0 flex-1 flex-col gap-3 pl-4">
									<div class="flex min-h-5 flex-wrap gap-1.5">
										{#if release.status}
											<Badge variant={helmStatusVariant(release.status)} class={helmStatusClass(release.status)}>
												{formatStatusLabel(release.status)}
											</Badge>
										{/if}
									</div>
									<div
										class="truncate rounded-md border bg-surface-0 px-2.5 py-2 text-xs text-muted-foreground"
										title={release.storageKind + ":" + release.storageName}
									>
										{release.storageKind} · {release.storageName}
									</div>
									<div class="grid grid-cols-2 gap-1.5">
										<div class="min-w-0 rounded-md border bg-surface-0 px-2.5 py-1.5">
											<div class="text-[0.62rem] text-muted-foreground">Chart</div>
											<div class="truncate text-xs font-medium" title={release.chart ?? "-"}>{release.chart ?? "-"}</div>
										</div>
										<div class="min-w-0 rounded-md border bg-surface-0 px-2.5 py-1.5">
											<div class="text-[0.62rem] text-muted-foreground">App version</div>
											<div class="truncate text-xs font-medium" title={release.appVersion ?? "-"}>{release.appVersion ?? "-"}</div>
										</div>
										<div class="min-w-0 rounded-md border bg-surface-0 px-2.5 py-1.5">
											<div class="text-[0.62rem] text-muted-foreground">Revision</div>
											<div class="truncate text-xs font-medium tabular-nums">{release.revision ?? "-"}</div>
										</div>
										<div class="min-w-0 rounded-md border bg-surface-0 px-2.5 py-1.5">
											<div class="text-[0.62rem] text-muted-foreground">Age</div>
											<div class="truncate text-xs font-medium tabular-nums">{release.age}</div>
										</div>
									</div>
								</CardContent>
								<CardFooter class="mt-auto flex items-center gap-2 pl-4">
									<Button type="button" variant="ghost" size="sm" onclick={() => list.select(release)}>
										Details
									</Button>
									<Button
										type="button"
										variant="outline"
										size="sm"
										class="ml-auto"
										onclick={() => onOpenResources(release.namespace, release.name)}
									>
										View resources
									</Button>
								</CardFooter>
							</Card>
						{/each}
					</div>
				{:else}
					<div class="overflow-x-auto rounded-lg border">
						<div class="grid min-w-[76rem] grid-cols-[minmax(12rem,1.1fr)_8rem_minmax(14rem,1.2fr)_minmax(9rem,0.9fr)_minmax(7rem,0.7fr)_5rem_5rem_9rem] items-center gap-3 border-b bg-muted/30 px-3 py-2 text-[0.65rem] font-semibold uppercase tracking-wide text-muted-foreground">
							<span>Name</span>
							<span>Status</span>
							<span>Storage</span>
							<span>Chart</span>
							<span>App version</span>
							<span>Revision</span>
							<span>Age</span>
							<span class="text-right">Actions</span>
						</div>
						{#each filteredHelmReleases as release (helmReleaseKey(release))}
							<div
								class={cn(
									"relative grid min-w-[76rem] grid-cols-[minmax(12rem,1.1fr)_8rem_minmax(14rem,1.2fr)_minmax(9rem,0.9fr)_minmax(7rem,0.7fr)_5rem_5rem_9rem] items-center gap-3 border-b bg-surface-1 px-3 py-2.5 last:border-b-0",
									helmReleaseKey(release) === selectedHelmReleaseKey
										? "bg-accent ring-1 ring-ring"
										: "hover:bg-accent/40",
								)}
							>
								<div class={cn("absolute inset-y-0 left-0 w-1", helmStatusRailClass(release.status))}></div>
								<div class="min-w-0 pl-1">
									<div class="truncate font-heading text-sm font-medium">{release.name}</div>
									<div class="truncate text-xs text-muted-foreground">{release.namespace}</div>
								</div>
								<div>
									{#if release.status}
										<Badge variant={helmStatusVariant(release.status)} class={helmStatusClass(release.status)}>
											{formatStatusLabel(release.status)}
										</Badge>
									{:else}
										<span class="text-muted-foreground">-</span>
									{/if}
								</div>
								<div class="min-w-0">
									<div class="truncate text-sm">{release.storageKind}</div>
									<div class="truncate text-xs text-muted-foreground" title={release.storageName}>{release.storageName}</div>
								</div>
								<span class="truncate" title={release.chart ?? "-"}>{release.chart ?? "-"}</span>
								<span class="truncate" title={release.appVersion ?? "-"}>{release.appVersion ?? "-"}</span>
								<span class="tabular-nums">{release.revision ?? "-"}</span>
								<span class="tabular-nums">{release.age}</span>
								<div class="flex justify-end gap-1">
									<Button type="button" variant="ghost" size="sm" onclick={() => list.select(release)}>
										Details
									</Button>
									<Button
										type="button"
										variant="outline"
										size="sm"
										onclick={() => onOpenResources(release.namespace, release.name)}
									>
										Resources
									</Button>
								</div>
							</div>
						{/each}
					</div>
				{/if}
			</section>
		</div>

		{#if selectedHelmRelease}
		<Card size="sm" elevation="flat">
			<CardHeader class="flex flex-row items-start justify-between gap-3">
				<div class="min-w-0">
					<CardTitle>{selectedHelmRelease?.name ?? "Release details"}</CardTitle>
					<CardDescription>
						{#if selectedHelmRelease}
							{selectedHelmRelease.namespace} / {selectedHelmRelease.storageKind}:{selectedHelmRelease.storageName}
						{:else}
							Select a release to inspect manifest, values, and storage metadata.
						{/if}
					</CardDescription>
				</div>
				{#if selectedHelmRelease}
					<div class="flex shrink-0 items-center gap-2">
						<Button type="button" variant="outline" size="sm" onclick={() => onOpenResources(selectedHelmRelease?.namespace, selectedHelmRelease?.name)}>
							<ExternalLink data-icon="inline-start" />
							View resources
						</Button>
						<Button type="button" variant="ghost" size="icon" aria-label="Close Helm details" onclick={() => list.select(null)}>
							<X />
						</Button>
					</div>
				{/if}
			</CardHeader>
			<CardContent>
				{#if helmDetailsQuery.isPending}
			<p class="inline-flex items-center gap-2 text-sm text-muted-foreground">
				<Spinner class="size-4" />
				Loading Helm release details...
			</p>
				{:else if helmDetailsQuery.isError}
					<FriendlyError
						error={helmDetailsQuery.error}
						context={{
							operation: "detailsLoad",
							fallbackTitle: "Helm details unavailable",
						}}
					/>
				{:else if helmDetailsQuery.data}
					{@const details = helmDetailsQuery.data}
					<div class="grid gap-4 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
						<div class="space-y-4">
							<div class="grid grid-cols-2 gap-2 text-sm">
								<div>
									<p class="text-xs uppercase text-muted-foreground">Chart</p>
									<p class="truncate font-medium">{details.summary.chart ?? "-"}</p>
								</div>
								<div>
									<p class="text-xs uppercase text-muted-foreground">App version</p>
									<p class="truncate font-medium">{details.summary.appVersion ?? "-"}</p>
								</div>
								<div>
									<p class="text-xs uppercase text-muted-foreground">Revision</p>
									<p class="font-medium">{details.summary.revision ?? "-"}</p>
								</div>
								<div>
									<p class="text-xs uppercase text-muted-foreground">Status</p>
									<p class="font-medium">{formatStatusLabel(details.summary.status ?? "unknown")}</p>
								</div>
								<div>
									<p class="text-xs uppercase text-muted-foreground">Updated</p>
									<p class="truncate font-medium">{details.summary.updatedAt ?? details.summary.age}</p>
								</div>
								<div>
									<p class="text-xs uppercase text-muted-foreground">Values</p>
									<p class="font-medium">
										{details.valuesSummary.hasValues ? `${details.valuesSummary.valueCount} values` : "No values"}
									</p>
								</div>
							</div>
							<div>
								<p class="mb-2 text-xs uppercase text-muted-foreground">Top-level values</p>
								<div class="flex flex-wrap gap-1">
									{#if details.valuesSummary.topLevelKeys.length > 0}
										{#each details.valuesSummary.topLevelKeys as key}
											<Badge variant="outline">{key}</Badge>
										{/each}
									{:else}
										<span class="text-sm text-muted-foreground">None decoded</span>
									{/if}
								</div>
							</div>
							<div>
								<p class="mb-2 text-xs uppercase text-muted-foreground">
									Manifest resources ({details.manifestSummary.resourceCount})
								</p>
				<SimpleTable
					headers={["Kind", "Name", "Namespace", "API"]}
					rows={details.manifestSummary.resources.map((resource: HelmManifestResourceSummary) => [
						resource.kind ?? "-",
						resource.name ?? "-",
						resource.namespace ?? "-",
						resource.apiVersion ?? "-",
					])}
									empty="No manifest resources decoded"
								/>
								{#if details.manifestSummary.truncated}
									<p class="mt-2 text-xs text-muted-foreground">Manifest list truncated by backend.</p>
								{/if}
							</div>
							<div>
								<div class="mb-2 flex items-center justify-between gap-2">
									<p class="text-xs uppercase text-muted-foreground">Reconciliation</p>
									{#if helmReconciliationQuery.data}
										{@const totals = helmReconciliationQuery.data.totals}
										<span class="text-xs text-muted-foreground">
											{totals.tracked} tracked, {totals.missing} missing, {totals.labelOnly} label-only
										</span>
									{/if}
								</div>
								{#if helmReconciliationQuery.isPending}
									<p class="inline-flex items-center gap-2 text-sm text-muted-foreground">
										<Spinner class="size-4" />
										Loading reconciliation...
								</p>
							{:else if helmReconciliationQuery.isError}
								<FriendlyError
									mode="compact"
									error={helmReconciliationQuery.error}
									context={{
										operation: "resourcesLoad",
										fallbackTitle: "Helm reconciliation unavailable",
										partial: true,
									}}
								/>
								{:else if helmReconciliationQuery.data}
									{#if helmReconciliationQuery.data.warnings.length > 0}
										<Alert class="mb-3">
											<AlertTitle>Reconciliation notes</AlertTitle>
											<AlertDescription>
												<ul class="flex list-disc flex-col gap-1 pl-4">
													{#each helmReconciliationQuery.data.warnings as warning}
														<li>{warning}</li>
													{/each}
												</ul>
											</AlertDescription>
										</Alert>
									{/if}
									<div class="rounded-md border">
										<Table class="min-w-[760px] table-fixed text-xs">
											<TableHeader>
												<TableRow>
													<TableHead class="px-3 py-2 font-semibold uppercase text-muted-foreground">Resource</TableHead>
													<TableHead class="px-3 py-2 font-semibold uppercase text-muted-foreground">Namespace</TableHead>
													<TableHead class="px-3 py-2 font-semibold uppercase text-muted-foreground">Status</TableHead>
													<TableHead class="px-3 py-2 font-semibold uppercase text-muted-foreground">Source</TableHead>
													<TableHead class="px-3 py-2 font-semibold uppercase text-muted-foreground">Message</TableHead>
												</TableRow>
											</TableHeader>
											<TableBody>
												{#if helmReconciliationRows.length === 0}
													<TableRow>
														<TableCell class="px-3 py-8 text-center text-muted-foreground" colspan="5">
															No manifest or explicit Helm-labeled live resources were found.
														</TableCell>
													</TableRow>
												{:else}
													{#each helmReconciliationRows as resource}
														<TableRow>
															<TableCell class="truncate px-3 py-2">{helmReconciliationResourceLabel(resource)}</TableCell>
															<TableCell class="truncate px-3 py-2">{resource.namespace ?? "-"}</TableCell>
															<TableCell class="px-3 py-2">
																<Badge variant="outline" class={helmReconciliationClass(resource.status)}>
																	{helmReconciliationStatusLabel(resource.status)}
																</Badge>
															</TableCell>
															<TableCell class="truncate px-3 py-2">{helmReconciliationSource(resource)}</TableCell>
															<TableCell class="truncate px-3 py-2">{resource.statusMessage}</TableCell>
														</TableRow>
													{/each}
												{/if}
											</TableBody>
										</Table>
									</div>
								{/if}
							</div>
						</div>
						<div class="min-w-0">
							<p class="mb-2 text-xs uppercase text-muted-foreground">Rendered YAML</p>
							<pre class="max-h-80 overflow-auto rounded-md border bg-muted/30 p-3 text-xs leading-relaxed">{details.yaml || "No YAML returned"}</pre>
						</div>
					</div>
				{/if}
			</CardContent>
		</Card>
			{/if}
	</div>
</SurfaceFrame>
