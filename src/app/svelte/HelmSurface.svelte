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
		CardHeader,
		CardTitle,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Input,
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
	} from "@/features/helm/helpers";
	import type { HelmManifestResourceSummary } from "@/lib/types";
	import { formatStatusLabel } from "@/lib/utils";
	import SimpleTable from "./SimpleTable.svelte";
	import StatGrid from "@/components/StatGrid.svelte";
	import SurfaceFrame from "@/components/SurfaceFrame.svelte";

	let {
		helmQuery,
		groupedHelmReleases,
		filteredHelmReleases,
		helmSearch = $bindable(""),
		selectedHelmRelease = $bindable(null),
		selectedHelmReleaseKey,
		helmDetailsQuery,
		helmReconciliationQuery,
		helmReconciliationRows,
		onOpenResources,
		helmStatusVariant,
		helmReconciliationClass,
		helmReconciliationSource,
	} = $props();
</script>

<SurfaceFrame icon={Package} title="Helm Releases" query={helmQuery} errorLabel="Helm releases unavailable">
		<StatGrid
			stats={[
				["Releases", helmQuery.data?.length ?? 0],
				["Namespaces", groupedHelmReleases.length],
				["Filtered", filteredHelmReleases.length],
			]}
		/>
		<div class="flex items-center gap-2">
			<div class="relative min-w-0 flex-1">
				<Search
					class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
				/>
				<Input
					class="h-8 pl-8"
					bind:value={helmSearch}
					placeholder="Search by release, namespace, chart, app version..."
					aria-label="Search Helm releases"
				/>
			</div>
			{#if helmSearch}
				<Button type="button" variant="outline" size="sm" onclick={() => (helmSearch = "")}>
					<X data-icon="inline-start" />
					Clear
				</Button>
			{/if}
		</div>
		<Card size="sm" elevation="flat">
			<CardContent class="p-0">
				<Table class="min-w-[900px] table-fixed text-sm">
					<TableHeader>
						<TableRow>
							{#each ["Release", "Namespace", "Chart", "App Version", "Revision", "Status", "Storage", "Updated"] as header}
								<TableHead class="px-3 py-2 text-xs font-semibold uppercase text-muted-foreground">
									{header}
								</TableHead>
							{/each}
						</TableRow>
					</TableHeader>
					<TableBody>
						{#if filteredHelmReleases.length === 0}
							<TableRow>
								<TableCell class="px-3 py-8 text-center text-muted-foreground" colspan="8">
									No Helm releases found
								</TableCell>
							</TableRow>
						{:else}
							{#each groupedHelmReleases as group (group.namespace)}
								<TableRow class="bg-muted/35 hover:bg-muted/35">
									<TableCell
										class="px-3 py-2 text-xs font-semibold uppercase text-muted-foreground"
										colspan="8"
									>
										{group.namespace} ({group.releases.length})
									</TableCell>
								</TableRow>
								{#each group.releases as release (helmReleaseKey(release))}
									<TableRow
										class={helmReleaseKey(release) === selectedHelmReleaseKey
											? "bg-accent hover:bg-accent"
											: "cursor-pointer hover:bg-accent/60"}
										tabindex="0"
										role="button"
										aria-pressed={helmReleaseKey(release) === selectedHelmReleaseKey}
										onclick={() => (selectedHelmRelease = release)}
										onkeydown={(event: KeyboardEvent) => {
											if (event.key === "Enter" || event.key === " ") {
												event.preventDefault();
												selectedHelmRelease = release;
											}
											}}
									>
										<TableCell class="truncate px-3 py-2 font-medium">{release.name}</TableCell>
										<TableCell class="truncate px-3 py-2">{release.namespace}</TableCell>
										<TableCell class="truncate px-3 py-2">{release.chart ?? "-"}</TableCell>
										<TableCell class="truncate px-3 py-2">{release.appVersion ?? "-"}</TableCell>
										<TableCell class="truncate px-3 py-2">{release.revision ?? "-"}</TableCell>
										<TableCell class="truncate px-3 py-2">
											{#if release.status}
												<Badge variant={helmStatusVariant(release.status)}>
													{formatStatusLabel(release.status)}
												</Badge>
											{:else}
												-
											{/if}
										</TableCell>
										<TableCell class="truncate px-3 py-2">{release.storageKind}</TableCell>
										<TableCell class="truncate px-3 py-2">{release.age}</TableCell>
									</TableRow>
								{/each}
							{/each}
						{/if}
					</TableBody>
				</Table>
			</CardContent>
		</Card>
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
						<Button type="button" variant="ghost" size="icon" aria-label="Close Helm details" onclick={() => (selectedHelmRelease = null)}>
							<X />
						</Button>
					</div>
				{/if}
			</CardHeader>
			<CardContent>
				{#if !selectedHelmRelease}
					<Empty>
						<EmptyHeader>
							<EmptyTitle>No release selected</EmptyTitle>
							<EmptyDescription>Use the Helm table to open release details.</EmptyDescription>
						</EmptyHeader>
			</Empty>
		{:else if helmDetailsQuery.isPending}
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
	</SurfaceFrame>
