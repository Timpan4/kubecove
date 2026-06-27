<script lang="ts">
	import { ExternalLink } from "lucide-svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Badge,
		Button,
		Spinner,
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow,
		TabsContent,
	} from "@/components/ui/svelte";
	import {
		conditionStatusTone,
		getErrorMessage,
		resourceReadyLabel,
		resourceReadyTone,
		resourceStatusTone,
	} from "./helpers";
	import type { IncidentSignal, IncidentSignalValuePart } from "./helpers";

	let {
		detailsQuery,
		eventsQuery,
		detailResource,
		conditionRows,
		containerRows,
		metadataRows,
		statusRows,
		curatedMetadata,
		visibleMetadataLabels,
		visibleMetadataAnnotations,
		topIncidentSignals,
		incidentTimeline,
		onOpenHelmRelease,
		metadataLabelsExpanded = $bindable(false),
		metadataAnnotationsExpanded = $bindable(false),
		detailStatusLabel,
		toneBadgeVariant,
		compactToneBadgeClass,
		restartsTone,
		incidentToneClass,
		formatFullTimestamp,
		metadataBadgeStyle,
		containerTone,
		containerReadyLabel,
		containerStateLabel,
	} = $props();

	function signalValueParts(signal: IncidentSignal): IncidentSignalValuePart[] {
		return signal.valueParts ?? [{ kind: "text", text: signal.value }];
	}
</script>

<TabsContent value="details" class="min-h-0">
		{#if detailsQuery.isPending}
			<div class="flex min-h-32 items-center justify-center gap-2 text-muted-foreground">
				<Spinner />
				<span>Loading details</span>
			</div>
		{:else if detailsQuery.isError}
			<Alert variant="destructive">
				<AlertTitle>Failed to load details</AlertTitle>
				<AlertDescription>{getErrorMessage(detailsQuery.error)}</AlertDescription>
			</Alert>
		{:else}
				<div class="flex flex-col gap-3">
					<div class="flex flex-wrap gap-2">
						<Badge variant="outline">{detailResource.kind}</Badge>
						<Badge variant="outline">{detailResource.namespace ?? "cluster"}</Badge>
						{#if detailStatusLabel(detailResource)}
							<Badge
								variant={toneBadgeVariant(resourceStatusTone(detailStatusLabel(detailResource)))}
								class={compactToneBadgeClass(resourceStatusTone(detailStatusLabel(detailResource)))}
							>
								{detailStatusLabel(detailResource)}
							</Badge>
						{/if}
						{#if resourceReadyLabel(detailResource)}
							<Badge
								variant={toneBadgeVariant(resourceReadyTone(detailResource))}
								class={compactToneBadgeClass(resourceReadyTone(detailResource))}
							>
								Ready {resourceReadyLabel(detailResource)}
							</Badge>
						{/if}
						<Badge
							variant={toneBadgeVariant(restartsTone(detailResource.restarts))}
							class={compactToneBadgeClass(restartsTone(detailResource.restarts))}
						>
							Restarts {detailResource.restarts ?? 0}
						</Badge>
						{#if detailResource.ownerRef}
							<Badge variant="outline" class="max-w-full truncate rounded-full px-2 py-0 text-[0.6875rem] shadow-none">
								Owner {detailResource.ownerRef}
							</Badge>
						{/if}
						{#if detailResource.helmRelease && onOpenHelmRelease}
							<Button
							type="button"
							variant="outline"
							size="sm"
							onclick={() =>
								onOpenHelmRelease?.(detailResource.helmRelease ?? "", detailResource.namespace)}
						>
							<ExternalLink data-icon="inline-start" />
							Open Helm release
						</Button>
					{/if}
				</div>
				<section class="rounded-md border border-primary/30 bg-primary/5 p-3">
					<div class="flex flex-wrap items-start justify-between gap-2">
						<div class="min-w-0">
							<div class="text-xs font-semibold uppercase text-muted-foreground">Incident summary</div>
							<div class="mt-1 break-words text-sm font-semibold">
								{detailResource.kind}/{detailResource.name}
							</div>
						</div>
						<div class="flex flex-wrap justify-end gap-1.5">
							{#if detailResource.status}
								<Badge
									variant={toneBadgeVariant(resourceStatusTone(detailResource.status))}
									class={compactToneBadgeClass(resourceStatusTone(detailResource.status))}
								>
									{detailResource.status}
								</Badge>
							{/if}
							{#if detailResource.ready}
								<Badge
									variant={toneBadgeVariant(resourceReadyTone(detailResource))}
									class={compactToneBadgeClass(resourceReadyTone(detailResource))}
								>
									Ready {resourceReadyLabel(detailResource)}
								</Badge>
							{/if}
							{#if detailResource.restarts !== undefined && detailResource.restarts > 0}
								<Badge
									variant={toneBadgeVariant(restartsTone(detailResource.restarts))}
									class={compactToneBadgeClass(restartsTone(detailResource.restarts))}
								>
									{detailResource.restarts} restarts
								</Badge>
							{/if}
						</div>
					</div>
					<div class="mt-3 flex flex-col gap-2">
						{#if topIncidentSignals.length === 0}
							<div class="rounded-md border bg-surface-0 px-3 py-2 text-xs text-muted-foreground">
								{eventsQuery.isPending
									? "Checking events and status signals..."
									: eventsQuery.isError
										? "Status loaded; event signals are unavailable."
										: "No active incident signals for this resource."}
							</div>
						{:else}
							{#each topIncidentSignals as signal (signal.id)}
								<div
									class={`flex items-start justify-between gap-2 rounded-md border border-l-4 px-2.5 py-1.5 ${incidentToneClass(signal.tone)}`}
								>
							<div class="min-w-0">
								<div class="text-xs font-semibold">{signal.label}</div>
								<div class="mt-1 break-words text-xs leading-snug text-muted-foreground">
									{#each signalValueParts(signal) as part}
										{#if part.kind === "timestamp"}
											<time datetime={part.value} title={formatFullTimestamp(part.value)}>
												{formatFullTimestamp(part.value)}
											</time>
										{:else}
											{part.text}
										{/if}
									{/each}
								</div>
							</div>
									<Badge
										variant={toneBadgeVariant(signal.tone)}
										class={compactToneBadgeClass(signal.tone)}
									>
										{signal.source}
									</Badge>
								</div>
							{/each}
						{/if}
					</div>
				</section>
				<section class="rounded-md border bg-background/30 p-3">
					<div class="mb-2 text-xs font-semibold uppercase text-muted-foreground">Timeline</div>
					{#if incidentTimeline.length === 0}
						<div class="rounded-md border bg-background/70 px-3 py-2 text-xs text-muted-foreground">
							No incident timeline entries for this resource.
						</div>
					{:else}
						<div class="flex flex-col gap-2">
							{#each incidentTimeline as item (item.id)}
								<div
									class={`rounded-md border border-l-4 px-2.5 py-2 ${incidentToneClass(item.tone)}`}
								>
									<div class="flex items-start justify-between gap-2">
										<div class="min-w-0">
											<div class="text-xs font-semibold">{item.title}</div>
											{#if item.detail}
												<div class="mt-1 break-words text-xs leading-snug text-muted-foreground">
													{item.detail}
												</div>
											{/if}
										</div>
									<Badge
										variant={toneBadgeVariant(item.tone)}
										class={compactToneBadgeClass(item.tone)}
									>
										{item.source}
									</Badge>
								</div>
								{#if item.timestamp}
									<div class="mt-1 text-[0.6875rem] text-muted-foreground">
										<time datetime={item.timestamp} title={formatFullTimestamp(item.timestamp)}>
											{formatFullTimestamp(item.timestamp)}
										</time>
									</div>
								{/if}
								</div>
							{/each}
						</div>
					{/if}
				</section>
				{#if conditionRows.length > 0}
					<section class="flex flex-col gap-2">
						<div class="text-xs font-semibold uppercase text-muted-foreground">Conditions</div>
						<div class="overflow-hidden rounded-md border bg-background/80">
							<Table>
								<TableHeader>
									<TableRow>
										<TableHead>Condition</TableHead>
										<TableHead>Status</TableHead>
										<TableHead>Reason</TableHead>
									<TableHead>Last transition</TableHead>
									</TableRow>
								</TableHeader>
								<TableBody>
									{#each conditionRows as condition (condition.type)}
										{@const tone = conditionStatusTone(condition, detailResource)}
										<TableRow>
											<TableCell>{condition.type}</TableCell>
											<TableCell>
												<Badge
													variant={toneBadgeVariant(tone)}
													class={compactToneBadgeClass(tone)}
												>
													{condition.status}
												</Badge>
											</TableCell>
									<TableCell class="max-w-72 whitespace-normal break-words">
										<div>{condition.reason ?? "-"}</div>
										{#if condition.message}
											<div class="mt-1 text-muted-foreground">{condition.message}</div>
										{/if}
									</TableCell>
											<TableCell class="whitespace-nowrap">
												{#if condition.lastTransitionTime}
													<time
														datetime={condition.lastTransitionTime}
														title={formatFullTimestamp(condition.lastTransitionTime)}
													>
														{formatFullTimestamp(condition.lastTransitionTime)}
													</time>
												{:else}
													-
												{/if}
											</TableCell>
								</TableRow>
									{/each}
								</TableBody>
							</Table>
						</div>
					</section>
				{/if}
				<section class="@container rounded-md border bg-background/30 p-3">
					<div class="mb-2 text-xs font-semibold uppercase text-muted-foreground">Metadata</div>
					<div class="grid gap-3 @md:grid-cols-2">
						<div class="rounded-md border bg-background/70 p-3">
							<div class="mb-2 text-xs font-semibold text-muted-foreground">Identity</div>
							<div class="grid text-xs">
								{#each curatedMetadata.identity as row (row.label)}
									<div class="grid grid-cols-[7rem_minmax(0,1fr)] gap-2 border-b py-1.5 last:border-b-0">
										<div class="text-muted-foreground">{row.label}</div>
										<div class="break-words font-semibold">{row.value}</div>
									</div>
								{/each}
							</div>
						</div>
						<div class="rounded-md border bg-background/70 p-3">
							<div class="mb-2 text-xs font-semibold text-muted-foreground">Lifecycle</div>
							<div class="grid text-xs">
								{#each curatedMetadata.lifecycle as row (row.label)}
									<div class="grid grid-cols-[7rem_minmax(0,1fr)] gap-2 border-b py-1.5 last:border-b-0">
										<div class="text-muted-foreground">{row.label}</div>
										<div class="break-words font-semibold">
											{#if (row.label === "Created" || row.label === "Deletion") && row.value !== "not scheduled"}
												<time datetime={row.value} title={formatFullTimestamp(row.value)}>
													{formatFullTimestamp(row.value)}
												</time>
											{:else if row.label === "Deletion" && row.value === "not scheduled"}
												<Badge
													variant={toneBadgeVariant("success")}
													class={compactToneBadgeClass("success")}
												>
													{row.value}
												</Badge>
											{:else}
												{row.value}
											{/if}
										</div>
									</div>
								{/each}
							</div>
						</div>
						<div class="rounded-md border bg-background/70 p-3">
							<div class="mb-2 text-xs font-semibold text-muted-foreground">Naming</div>
							<div class="grid text-xs">
								{#each curatedMetadata.naming as row (row.label)}
									<div class="grid grid-cols-[7rem_minmax(0,1fr)] gap-2 border-b py-1.5 last:border-b-0">
										<div class="text-muted-foreground">{row.label}</div>
										<div class="break-words font-semibold">{row.value}</div>
									</div>
								{/each}
							</div>
						</div>
						<div class="rounded-md border bg-background/70 p-3">
							<div class="mb-2 text-xs font-semibold text-muted-foreground">Ownership</div>
							{#if curatedMetadata.ownership.length === 0}
								<div class="text-xs text-muted-foreground">None detected</div>
							{:else}
								<div class="grid text-xs">
							{#each curatedMetadata.ownership as row (row.label)}
								<div class="grid grid-cols-[7rem_minmax(0,1fr)] gap-2 border-b py-1.5 last:border-b-0">
									<div class="text-muted-foreground">{row.label}</div>
									<div class="break-words font-semibold">
										{#if row.label === "Helm" && onOpenHelmRelease}
											<Button
												type="button"
												variant="outline"
												size="sm"
												onclick={() => onOpenHelmRelease?.(row.value, detailResource.namespace)}
											>
												<ExternalLink data-icon="inline-start" />
												{row.value}
											</Button>
										{:else}
											{row.value}
										{/if}
									</div>
								</div>
							{/each}
								</div>
							{/if}
						</div>
						<div class="rounded-md border bg-background/70 p-3">
							<div class="mb-2 text-xs font-semibold text-muted-foreground">Labels</div>
							{#if curatedMetadata.labels.length === 0}
								<div class="text-xs text-muted-foreground">None</div>
							{:else}
								<div class="flex w-full min-w-0 flex-col items-start gap-1.5">
									{#each visibleMetadataLabels.badges as item (item.key)}
										<Badge
											variant="outline"
											class="h-auto min-h-6 min-w-0 max-w-full justify-start overflow-hidden rounded-sm px-2 py-1 text-left text-[0.6875rem] leading-snug whitespace-nowrap shadow-none"
											style={metadataBadgeStyle(item.key)}
											title={item.value}
										>
											<span class="shrink-0 font-semibold">{item.key}</span>
											<span class="shrink-0 opacity-75">=</span>
											<span class="min-w-0 flex-1 truncate" aria-label={`${item.key} value`}>
												{item.value}
											</span>
										</Badge>
									{/each}
								</div>
								{#if visibleMetadataLabels.hiddenCount > 0 || metadataLabelsExpanded}
									<Button
										type="button"
										variant="ghost"
										size="sm"
										class="mt-2 h-7 px-2 text-xs"
										onclick={() => (metadataLabelsExpanded = !metadataLabelsExpanded)}
									>
										{#if metadataLabelsExpanded}
											Show less
										{:else}
											Show {visibleMetadataLabels.hiddenCount} more
										{/if}
									</Button>
								{/if}
							{/if}
						</div>
						<div class="rounded-md border bg-background/70 p-3">
							<div class="mb-2 text-xs font-semibold text-muted-foreground">Annotations and Management</div>
							<div class="grid text-xs">
								<div class="grid grid-cols-[7rem_minmax(0,1fr)] gap-2 border-b py-1.5">
									<div class="text-muted-foreground">Annotations</div>
									<div class="break-words font-semibold">{curatedMetadata.annotationCount}</div>
								</div>
								{#each curatedMetadata.management as row (row.label)}
									<div class="grid grid-cols-[7rem_minmax(0,1fr)] gap-2 border-b py-1.5 last:border-b-0">
										<div class="text-muted-foreground">{row.label}</div>
										<div class="break-words font-semibold">{row.value}</div>
									</div>
								{/each}
							</div>
							{#if visibleMetadataAnnotations.badges.length > 0}
								<div class="mt-2 flex w-full min-w-0 flex-col items-start gap-1.5">
									{#each visibleMetadataAnnotations.badges as item (item.key)}
										<Badge
											variant="outline"
											class="h-auto min-h-6 min-w-0 max-w-full justify-start overflow-hidden rounded-sm px-2 py-1 text-left text-[0.6875rem] leading-snug whitespace-nowrap shadow-none"
											style={metadataBadgeStyle(item.key)}
											title={item.value}
										>
											<span class="shrink-0 font-semibold">{item.key}</span>
											<span class="shrink-0 opacity-75">=</span>
											<span class="min-w-0 flex-1 truncate" aria-label={`${item.key} value`}>
												{item.value}
											</span>
										</Badge>
									{/each}
								</div>
								{#if visibleMetadataAnnotations.hiddenCount > 0 || metadataAnnotationsExpanded}
									<Button
										type="button"
										variant="ghost"
										size="sm"
										class="mt-2 h-7 px-2 text-xs"
										onclick={() => (metadataAnnotationsExpanded = !metadataAnnotationsExpanded)}
									>
										{#if metadataAnnotationsExpanded}
											Show less
										{:else}
											Show {visibleMetadataAnnotations.hiddenCount} more
										{/if}
									</Button>
								{/if}
							{/if}
							{#if curatedMetadata.annotationCount > curatedMetadata.annotations.length}
								<div class="mt-2 text-[0.6875rem] text-muted-foreground">
									{curatedMetadata.annotationCount - curatedMetadata.annotations.length} long or structured
									annotation{curatedMetadata.annotationCount - curatedMetadata.annotations.length === 1 ? "" : "s"}
									in Advanced metadata.
								</div>
							{/if}
						</div>
					</div>
				</section>
				{#if containerRows.length > 0}
					<section class="flex flex-col gap-2">
						<div class="text-xs font-semibold uppercase text-muted-foreground">Containers</div>
						<div class="overflow-hidden rounded-md border bg-background/80">
							<Table>
								<TableHeader>
									<TableRow>
										<TableHead>Container</TableHead>
										<TableHead>Ready</TableHead>
										<TableHead>Restarts</TableHead>
									<TableHead>State</TableHead>
									<TableHead>Last finished</TableHead>
									</TableRow>
								</TableHeader>
								<TableBody>
									{#each containerRows as container (`${container.type}:${container.name}`)}
										{@const tone = containerTone(container)}
										<TableRow>
											<TableCell>{container.name}</TableCell>
											<TableCell>
												<Badge
													variant={toneBadgeVariant(tone)}
													class={compactToneBadgeClass(tone)}
												>
													{containerReadyLabel(container)}
												</Badge>
											</TableCell>
											<TableCell>
												<Badge
													variant={toneBadgeVariant(restartsTone(container.restartCount))}
													class={compactToneBadgeClass(restartsTone(container.restartCount))}
												>
													{container.restartCount}
												</Badge>
											</TableCell>
								<TableCell class="max-w-72 whitespace-normal break-words">
									{containerStateLabel(container)}
								</TableCell>
											<TableCell class="whitespace-nowrap">
												{#if container.lastFinishedAt}
													<time
														datetime={container.lastFinishedAt}
														title={formatFullTimestamp(container.lastFinishedAt)}
													>
														{formatFullTimestamp(container.lastFinishedAt)}
													</time>
												{:else}
													-
												{/if}
											</TableCell>
										</TableRow>
									{/each}
								</TableBody>
							</Table>
						</div>
					</section>
				{/if}
				<details class="rounded-md border bg-background/30">
					<summary class="cursor-pointer px-3 py-2 text-xs font-semibold uppercase text-muted-foreground">
						Advanced metadata
					</summary>
					<div class="overflow-hidden border-t bg-background/80">
						<Table>
							<TableHeader>
								<TableRow>
									<TableHead>Metadata</TableHead>
									<TableHead>Value</TableHead>
								</TableRow>
							</TableHeader>
							<TableBody>
								{#each metadataRows as row (row.key)}
									<TableRow>
										<TableCell>{row.key}</TableCell>
										<TableCell>
											<pre class="whitespace-pre-wrap break-words font-mono">{row.value}</pre>
										</TableCell>
									</TableRow>
								{/each}
							</TableBody>
						</Table>
					</div>
				</details>
				{#if statusRows.length > 0}
					<section class="flex flex-col gap-2">
						<div class="text-xs font-semibold uppercase text-muted-foreground">Status details</div>
						<div class="overflow-hidden rounded-md border bg-background/80">
							<Table>
								<TableHeader>
									<TableRow>
										<TableHead>Status</TableHead>
										<TableHead>Value</TableHead>
									</TableRow>
								</TableHeader>
								<TableBody>
									{#each statusRows as row (row.key)}
										<TableRow>
											<TableCell>{row.key}</TableCell>
											<TableCell>
												<pre class="whitespace-pre-wrap break-words font-mono">{row.value}</pre>
											</TableCell>
										</TableRow>
									{/each}
								</TableBody>
							</Table>
						</div>
					</section>
				{/if}
			</div>
		{/if}
	</TabsContent>
