<script lang="ts">
	import { AlertTriangle, ExternalLink, GitBranch, X } from "lucide-svelte";
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
		Spinner,
	} from "@/components/ui/svelte";
	import { extractArgoStatusInsights } from "@/features/argo/status-insights";
	import type { FluxResourceSummary } from "@/lib/types";
	import { formatStatusLabel } from "@/lib/utils";
	import { gitOpsSelectionCells, gitOpsSelectionKey } from "./gitOpsSurfaceModel";
	import SimpleTable from "./SimpleTable.svelte";
	import StatGrid from "./StatGrid.svelte";
	import SurfaceFrame from "./SurfaceFrame.svelte";

	let {
		gitOpsQuery,
		gitOpsProviderError,
		gitOpsListError,
		gitOpsUnavailableProvider,
		gitOpsTable,
		gitOpsSelections,
		selectedGitOpsItem = $bindable(null),
		selectedGitOpsItemKey,
		gitOpsDetailsQuery,
		errorMessage,
		openSelectedArgoApplicationResources,
		gitOpsSelectionLabel,
		gitOpsSelectionFacts,
		gitOpsStatusClass,
		gitOpsDetailsStatus,
	} = $props();
</script>

<SurfaceFrame icon={GitBranch} title="GitOps" query={gitOpsQuery} errorLabel="GitOps data unavailable">
		{@const data = gitOpsQuery.data}
		{#if data}
			{#if gitOpsProviderError}
				<Alert variant="destructive">
					<AlertTitle>Some GitOps providers could not be detected</AlertTitle>
					<AlertDescription>{errorMessage(gitOpsProviderError)}</AlertDescription>
				</Alert>
			{/if}
			{#if gitOpsListError}
				<Alert variant="destructive">
					<AlertTitle>Some GitOps resources could not load</AlertTitle>
					<AlertDescription>{errorMessage(gitOpsListError)}</AlertDescription>
				</Alert>
			{/if}
			<StatGrid
				stats={[
					["Argo apps", data.apps.length],
					["AppSets", data.appSets.length],
					["Projects", data.projects.length],
					["Flux resources", data.flux.length],
				]}
			/>
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
				<Card size="sm" elevation="flat">
					<CardHeader>
						<CardTitle>{gitOpsTable.title}</CardTitle>
						<CardDescription>Read-only GitOps inventory for current cluster context.</CardDescription>
					</CardHeader>
					<CardContent class="overflow-x-auto p-0">
						<table class="w-full min-w-[760px] table-fixed border-collapse text-sm">
							<thead>
								<tr>
									{#each gitOpsTable.headers as header}
										<th class="border-b px-3 py-2 text-left text-xs font-semibold uppercase text-muted-foreground">
											{header}
										</th>
									{/each}
								</tr>
							</thead>
							<tbody>
								{#if gitOpsSelections.length === 0}
									<tr>
										<td class="px-3 py-8 text-center text-muted-foreground" colspan={gitOpsTable.headers.length}>
											{gitOpsTable.empty}
										</td>
									</tr>
								{:else}
									{#each gitOpsSelections as item (gitOpsSelectionKey(item))}
										<tr
											class={gitOpsSelectionKey(item) === selectedGitOpsItemKey
												? "border-b bg-accent last:border-b-0"
												: "cursor-pointer border-b last:border-b-0 hover:bg-accent/60"}
											tabindex="0"
											role="button"
											aria-pressed={gitOpsSelectionKey(item) === selectedGitOpsItemKey}
											onclick={() => (selectedGitOpsItem = item)}
											onkeydown={(event: KeyboardEvent) => {
												if (event.key === "Enter" || event.key === " ") {
													event.preventDefault();
													selectedGitOpsItem = item;
												}
											}}
										>
											{#each gitOpsSelectionCells(item) as cell}
												<td class="truncate px-3 py-2">{cell}</td>
											{/each}
										</tr>
									{/each}
								{/if}
							</tbody>
						</table>
					</CardContent>
				</Card>
			{/if}
			<Card size="sm" elevation="flat">
				<CardHeader class="flex flex-row items-start justify-between gap-3">
					<div class="min-w-0">
						<CardTitle>{selectedGitOpsItem ? gitOpsSelectionLabel(selectedGitOpsItem) : "GitOps details"}</CardTitle>
						<CardDescription>
							{#if selectedGitOpsItem}
								{selectedGitOpsItem.item.namespace ?? "cluster-scoped"}
							{:else}
								Select a GitOps row to inspect details and YAML.
							{/if}
						</CardDescription>
					</div>
					{#if selectedGitOpsItem}
						<div class="flex shrink-0 items-center gap-2">
							{#if selectedGitOpsItem.type === "argoApp"}
								<Button type="button" variant="outline" size="sm" onclick={openSelectedArgoApplicationResources}>
									<ExternalLink data-icon="inline-start" />
									View resources
								</Button>
							{/if}
							<Button type="button" variant="ghost" size="icon" aria-label="Close GitOps details" onclick={() => (selectedGitOpsItem = null)}>
								<X />
							</Button>
						</div>
					{/if}
				</CardHeader>
				<CardContent>
					{#if !selectedGitOpsItem}
						<Empty>
							<EmptyHeader>
								<EmptyTitle>No GitOps item selected</EmptyTitle>
								<EmptyDescription>Use the GitOps table to open read-only details.</EmptyDescription>
							</EmptyHeader>
				</Empty>
			{:else if gitOpsDetailsQuery.isPending}
				<p class="inline-flex items-center gap-2 text-sm text-muted-foreground">
					<Spinner class="size-4" />
					Loading GitOps details...
				</p>
					{:else if gitOpsDetailsQuery.isError}
						<Alert variant="destructive">
							<AlertTriangle class="size-4" />
							<AlertTitle>GitOps details unavailable</AlertTitle>
							<AlertDescription>{errorMessage(gitOpsDetailsQuery.error)}</AlertDescription>
						</Alert>
					{:else if gitOpsDetailsQuery.data}
						{@const details = gitOpsDetailsQuery.data}
						<div class="grid gap-4 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
							<div class="space-y-4">
								<div class="grid grid-cols-2 gap-2 text-sm">
									{#each gitOpsSelectionFacts(selectedGitOpsItem) as [label, value]}
										<div>
											<p class="text-xs uppercase text-muted-foreground">{label}</p>
											<p class="truncate font-medium">{value}</p>
										</div>
									{/each}
								</div>
								{#if selectedGitOpsItem.type === "argoApp"}
									{@const app = selectedGitOpsItem.item}
									{@const insights = extractArgoStatusInsights(gitOpsDetailsStatus(details))}
									<div class="rounded-md border bg-surface-1 p-3">
										<p class="mb-2 text-xs uppercase text-muted-foreground">Sync & Health</p>
										<div class="grid gap-2 text-sm">
											<div class="flex items-center justify-between gap-3">
												<span class="text-xs font-medium text-muted-foreground">Sync Status</span>
												{#if app.syncStatus}
													<Badge variant="outline" class={gitOpsStatusClass(app.syncStatus)}>{formatStatusLabel(app.syncStatus)}</Badge>
												{/if}
											</div>
											<div class="flex items-center justify-between gap-3">
												<span class="text-xs font-medium text-muted-foreground">Health Status</span>
												{#if app.healthStatus}
													<Badge variant="outline" class={gitOpsStatusClass(app.healthStatus)}>{formatStatusLabel(app.healthStatus)}</Badge>
												{/if}
											</div>
											{#if insights.healthMessage}
												<div class="text-xs leading-relaxed text-muted-foreground [overflow-wrap:anywhere]">
													{insights.healthMessage}
												</div>
											{/if}
											{#if insights.healthTransitionTime}
												<div class="text-xs text-muted-foreground">Last transition {insights.healthTransitionTime}</div>
											{/if}
										</div>
									</div>
									{#if insights.unhealthyResources.length > 0}
										<div>
											<p class="mb-2 text-xs uppercase text-muted-foreground">Unhealthy Resources</p>
											<div class="grid gap-2">
												{#each insights.unhealthyResources as resource, index (`${resource.kind}:${resource.namespace}:${resource.name}:${index}`)}
													<div class="rounded-md border bg-surface-1 px-3 py-2">
														<div class="flex items-center justify-between gap-2">
															<span class="min-w-0 truncate text-xs font-semibold">{resource.kind ?? "Resource"}/{resource.name ?? "unknown"}</span>
															{#if resource.health}
																<Badge variant="outline" class={gitOpsStatusClass(resource.health)}>{formatStatusLabel(resource.health)}</Badge>
															{/if}
														</div>
														{#if resource.message}
															<div class="mt-1 text-xs leading-relaxed text-muted-foreground [overflow-wrap:anywhere]">{resource.message}</div>
														{/if}
													</div>
												{/each}
											</div>
										</div>
									{/if}
									{#if insights.conditions.length > 0}
										<div>
											<p class="mb-2 text-xs uppercase text-muted-foreground">Conditions</p>
											<div class="grid gap-2">
												{#each insights.conditions as condition, index (`${condition.type}:${index}`)}
													<div class="rounded-md border bg-surface-1 px-3 py-2">
														<div class="flex items-center justify-between gap-2">
															<span class="text-xs font-semibold">{condition.type}</span>
															{#if condition.lastTransitionTime}
																<span class="text-[0.68rem] text-muted-foreground">{condition.lastTransitionTime}</span>
															{/if}
														</div>
														{#if condition.message}
															<div class="mt-1 text-xs leading-relaxed text-muted-foreground [overflow-wrap:anywhere]">{condition.message}</div>
														{/if}
													</div>
												{/each}
											</div>
										</div>
									{/if}
									<div class="grid grid-cols-2 gap-2 text-sm">
										<div>
											<p class="text-xs uppercase text-muted-foreground">Destination</p>
											<p class="truncate font-medium">{app.destinationNamespace ?? app.destinationServer ?? "-"}</p>
										</div>
										<div>
											<p class="text-xs uppercase text-muted-foreground">Revision</p>
											<p class="truncate font-medium">{app.sourceRevision ?? "-"}</p>
										</div>
									</div>
								{/if}
								<div>
									<p class="mb-2 text-xs uppercase text-muted-foreground">Metadata</p>
									<pre class="max-h-64 overflow-auto rounded-md border bg-muted/30 p-3 text-xs leading-relaxed">{JSON.stringify(details.metadata, null, 2)}</pre>
								</div>
								{#if "status" in details && details.status}
									<div>
										<p class="mb-2 text-xs uppercase text-muted-foreground">Status</p>
										<pre class="max-h-64 overflow-auto rounded-md border bg-muted/30 p-3 text-xs leading-relaxed">{JSON.stringify(details.status, null, 2)}</pre>
									</div>
								{/if}
							</div>
							<div class="min-w-0">
								<p class="mb-2 text-xs uppercase text-muted-foreground">Rendered YAML</p>
								<pre class="max-h-80 overflow-auto rounded-md border bg-muted/30 p-3 text-xs leading-relaxed">{details.yaml || "No YAML returned"}</pre>
							</div>
						</div>
					{/if}
				</CardContent>
			</Card>
			{#if data.flux.length > 0 && gitOpsTable?.title.startsWith("Argo CD")}
				<SimpleTable
					headers={["Flux resource", "Namespace", "Ready", "Source", "Revision"]}
				rows={data.flux.map((item: FluxResourceSummary) => [
						`${item.resourceKind.kind}/${item.name}`,
						item.namespace ?? "-",
						item.readyStatus ?? "-",
						item.sourceName ?? "-",
						item.lastAppliedRevision ?? "-",
					])}
					empty="No Flux resources found"
				/>
			{/if}
		{/if}
	</SurfaceFrame>
