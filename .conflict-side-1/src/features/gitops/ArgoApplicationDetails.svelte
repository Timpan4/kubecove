<script lang="ts">
	import { yamlLanguage } from "@codemirror/lang-yaml";
	import { highlightTree, tagHighlighter, tags } from "@lezer/highlight";
	import {
		ChevronDown,
		CircleDot,
		GitCompareArrows,
		GitCommitHorizontal,
		RefreshCw,
		RotateCcw,
		TriangleAlert,
	} from "lucide-svelte";
	import { stringify } from "yaml";
	import {
		Badge,
		Button,
		Checkbox,
		Dialog,
		DialogContent,
		DialogDescription,
		DialogHeader,
		DialogTitle,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Field,
		FieldLabel,
		Input,
		Popover,
		PopoverContent,
		PopoverTrigger,
		SegmentedControl,
		Spinner,
		buttonClass,
	} from "@/components/ui/svelte";
	import YamlCodeEditor from "@/components/YamlCodeEditor.svelte";
	import {
		buildArgoResourceDiff,
		diffLineClassName,
		type UnifiedDiffLine,
	} from "@/features/resource-detail/yamlTabDiff";
	import { showToast } from "@/lib/toasts";
	import type { ArgoApplicationSummary, ArgoManagedResource, ResourceSummary } from "@/lib/types";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import {
		ARGO_SYNC_DEFAULTS,
		argoPhaseLabel,
		argoPrototypeResources,
		argoResourceCounts,
		createArgoPrototypeFixture,
		needsArgoSyncConfirmation,
		type ArgoDiffView,
		type ArgoPrototypeResource,
		type ArgoSyncSettings,
	} from "./argo-workspace-model";
	import {
		argoWorkspaceSessionKey,
		argoWorkspaceSessions,
		createArgoWorkspaceSession,
		ensureArgoWorkspaceSession,
		patchArgoWorkspaceSession,
	} from "./argo-workspace-session";

	let {
		resourceSummary,
		workspaceReadContext,
		active,
	}: {
		resourceSummary: ResourceSummary;
		workspaceReadContext: WorkspaceReadContext;
		active: boolean;
	} = $props();

	let expandedRemovalKeys = $state<string[]>([]);

	type VisibleDiffView = Extract<ArgoDiffView, "changes" | "target" | "live">;
	type YamlDiffSegment = { text: string; className: string };

	const yamlDiffHighlighter = tagHighlighter([
		{ tag: tags.keyword, class: "text-blue-700 dark:text-blue-300" },
		{ tag: [tags.atom, tags.bool], class: "text-violet-700 dark:text-violet-300" },
		{ tag: tags.number, class: "text-amber-700 dark:text-amber-300" },
		{ tag: [tags.string, tags.content], class: "text-emerald-700 dark:text-emerald-300" },
		{ tag: [tags.propertyName, tags.definition(tags.propertyName)], class: "text-sky-700 dark:text-sky-300" },
		{ tag: tags.comment, class: "text-muted-foreground italic" },
		{ tag: tags.punctuation, class: "text-muted-foreground" },
		{ tag: tags.operator, class: "text-rose-700 dark:text-rose-300" },
		{ tag: tags.invalid, class: "text-destructive" },
	]);

	const diffViewOptions: { value: VisibleDiffView; label: string }[] = [
		{ value: "changes", label: "Changes" },
		{ value: "target", label: "Desired" },
		{ value: "live", label: "Live" },
	];
	const clusterContext = $derived(workspaceReadContext.clusterContext);
	const workspaceId = $derived(workspaceReadContext.workspaceId);
	const sessionKey = $derived(
		argoWorkspaceSessionKey(
			clusterContext,
			workspaceId,
			resourceSummary.namespace,
			resourceSummary.name,
		),
	);
	const app = $derived(prototypeApplication(resourceSummary, clusterContext));
	const fixture = $derived(createArgoPrototypeFixture(app));
	const session = $derived($argoWorkspaceSessions[sessionKey] ?? createArgoWorkspaceSession());
	const phaseLabel = $derived(argoPhaseLabel(session.phase));
	const busy = $derived(session.phase !== "idle");
	const resources = $derived(argoPrototypeResources(fixture, session.synced));
	const counts = $derived(argoResourceCounts(resources));
	const attentionResources = $derived(
		resources.filter(
			(item) =>
				item.status === "OutOfSync" ||
				item.health === "Degraded" ||
				item.health === "Progressing" ||
				item.requiresPruning,
		),
	);
	const focusedResource = $derived(
		resources.find((item) => item.key === session.selectedResourceKey) ?? null,
	);
	const diffResources = $derived(focusedResource ? [focusedResource] : attentionResources);
	const visibleDiffView = $derived<VisibleDiffView>(
		session.diffView === "normalized"
			? "live"
			: session.diffView === "predicted"
				? "target"
				: session.diffView,
	);
	const selectedHistory = $derived(
		fixture.history.find((entry) => entry.id === session.selectedHistoryId) ?? fixture.history[0] ?? null,
	);

	let refreshMenuOpen = $state(false);
	let syncMenuOpen = $state(false);
	let advancedSync = $state<ArgoSyncSettings>({ ...ARGO_SYNC_DEFAULTS });
	let confirmationOpen = $state(false);
	let confirmationName = $state("");
	let pendingSync = $state<ArgoSyncSettings | null>(null);

	$effect(() => {
		ensureArgoWorkspaceSession(sessionKey);
	});

	function refresh(hard = false) {
		if (busy) return;
		refreshMenuOpen = false;
		patchArgoWorkspaceSession(sessionKey, {
			phase: "refreshing",
			message: hard ? "Invalidating manifest and repository caches…" : "Refreshing Application state…",
		});
		setTimeout(() => {
			patchArgoWorkspaceSession(sessionKey, {
				phase: "idle",
				message: hard
					? "Hard refresh completed from local prototype data."
					: "Application state refreshed from local prototype data.",
			});
			showToast({
				title: hard ? "Hard refresh complete" : "Application refreshed",
				description: hard
					? "Manifest and repository caches were simulated as refreshed."
					: "Health, sync, and managed-resource state were refreshed.",
				tone: "success",
			});
		}, 750);
	}

	function requestAdvancedSync() {
		syncMenuOpen = false;
		const requested = { ...advancedSync };
		if (needsArgoSyncConfirmation(requested, fixture.syncDefaults)) {
			pendingSync = requested;
			confirmationName = "";
			confirmationOpen = true;
			return;
		}
		startSync(requested);
	}

	function startSync(settings: ArgoSyncSettings = fixture.syncDefaults) {
		if (busy) return;
		confirmationOpen = false;
		pendingSync = null;
		confirmationName = "";
		patchArgoWorkspaceSession(sessionKey, {
			phase: "syncQueued",
			message: settings.dryRun
				? "Dry run queued with the selected options…"
				: "Sync queued with the selected options…",
			syncDraft: { ...settings },
		});
		setTimeout(() => {
			patchArgoWorkspaceSession(sessionKey, {
				phase: "syncing",
				healthStatus: settings.dryRun ? session.healthStatus : "Progressing",
				message: settings.dryRun
					? "Rendering and validating manifests…"
					: "Applying target revision to managed resources…",
			});
		}, 400);
		setTimeout(() => {
			if (settings.dryRun) {
				patchArgoWorkspaceSession(sessionKey, {
					phase: "idle",
					message: "Dry run completed. No live state was changed.",
				});
				showToast({
					title: "Dry run complete",
					description: "The selected sync options were validated without changing prototype state.",
					tone: "success",
				});
				return;
			}
			patchArgoWorkspaceSession(sessionKey, {
				phase: "idle",
				healthStatus: "Healthy",
				syncStatus: "Synced",
				message: `Synced revision ${settings.revision.trim() || fixture.resolvedRevision}.`,
				synced: true,
			});
			showToast({
				title: "Application synced",
				description: `${app.name} is Healthy and Synced in the local prototype.`,
				tone: "success",
			});
		}, 1_500);
	}

	function selectResource(item: ArgoPrototypeResource) {
		patchArgoWorkspaceSession(sessionKey, { selectedResourceKey: item.key });
	}

	function showAllChanges() {
		patchArgoWorkspaceSession(sessionKey, { selectedResourceKey: null });
	}

	function resourceDocument(item: ArgoPrototypeResource, view: ArgoDiffView): string {
		if (view === "target") return documentText(item.targetState);
		if (view === "live") return documentText(item.liveState);
		if (view === "normalized") return documentText(item.normalizedLiveState);
		return documentText(item.predictedLiveState);
	}

	function resourceDiffLines(item: ArgoPrototypeResource) {
		return buildArgoResourceDiff(documentText(item.targetState), documentText(item.liveState));
	}

	function resourceModified(item: ArgoPrototypeResource): boolean {
		return documentText(item.targetState) !== documentText(item.liveState);
	}

	function resourceAdded(item: ArgoPrototypeResource): boolean {
		return documentText(item.liveState) === "" && documentText(item.targetState) !== "";
	}

	function isRemovalOnlyResource(item: ArgoPrototypeResource): boolean {
		return item.requiresPruning === true && !resourceModified(item);
	}

	function removalDiffExpanded(item: ArgoPrototypeResource): boolean {
		return expandedRemovalKeys.includes(item.key);
	}

	function toggleRemovalDiff(item: ArgoPrototypeResource) {
		expandedRemovalKeys = removalDiffExpanded(item)
			? expandedRemovalKeys.filter((key) => key !== item.key)
			: [...expandedRemovalKeys, item.key];
	}

	function highlightedDiffSegments(line: UnifiedDiffLine): YamlDiffSegment[] {
		if (line.type !== "add" && line.type !== "remove" && line.type !== "context") {
			return [{ text: line.text, className: "" }];
		}

		const prefix = line.text.slice(0, 1);
		const source = line.text.slice(1);
		const segments: YamlDiffSegment[] = [{ text: prefix, className: "font-semibold" }];
		let cursor = 0;

		highlightTree(yamlLanguage.parser.parse(source), yamlDiffHighlighter, (from, to, className) => {
			if (from > cursor) segments.push({ text: source.slice(cursor, from), className: "" });
			segments.push({ text: source.slice(from, to), className });
			cursor = to;
		});
		if (cursor < source.length) {
			segments.push({ text: source.slice(cursor), className: "" });
		}
		return segments;
	}

	function resourceLabel(item: ArgoManagedResource): string {
		return `${item.kind ?? "Resource"} ${item.namespace ? `${item.namespace}/` : ""}${item.name ?? "unknown"}`;
	}

	function historyRevision(entry: { revision?: string | null; revisions: string[] }): string {
		return entry.revision ?? (entry.revisions.join(", ") || "Unknown revision");
	}

	function documentText(value: unknown): string {
		return value === undefined || value === null ? "" : stringify(value).trimEnd();
	}

	function prototypeApplication(summary: ResourceSummary, context: string): ArgoApplicationSummary {
		return {
			name: summary.name,
			cluster: context,
			namespace: summary.namespace ?? "argocd",
			project: "platform",
			syncStatus: "OutOfSync",
			healthStatus: "Degraded",
			destinationNamespace: "shop",
			destinationServer: "https://kubernetes.default.svc",
			sourceRepo: "https://github.com/example/platform-config.git",
			sourceRevision: "main",
			resourceNamespaces: ["shop"],
			trackedResourceCount: 5,
			age: summary.age ?? "12d",
		};
	}
</script>

<div class="@container flex min-h-0 flex-col gap-3" data-active={active}>
	{@render MissionBriefing()}
	{@render MissionDiffFeed()}
</div>

<Dialog open={confirmationOpen} onOpenChange={(open: boolean) => { confirmationOpen = open; if (!open) { pendingSync = null; confirmationName = ""; } }}>
	<DialogContent>
		<DialogHeader>
			<DialogTitle>Confirm advanced sync</DialogTitle>
			<DialogDescription>These choices go beyond the Application defaults. Type the Application name to continue in the local prototype.</DialogDescription>
		</DialogHeader>
		{#if pendingSync}
			<div class="grid grid-cols-[5rem_1fr] gap-x-3 gap-y-1 rounded-md border bg-muted/20 p-3 text-xs">
				<span class="text-muted-foreground">Revision</span><span>{pendingSync.revision || fixture.configuredRevision}</span>
				<span class="text-muted-foreground">Prune</span><span>{pendingSync.prune ? "Enabled" : "Disabled"}</span>
				<span class="text-muted-foreground">Dry run</span><span>{pendingSync.dryRun ? "Enabled" : "Disabled"}</span>
				<span class="text-muted-foreground">Force</span><span>{pendingSync.force ? "Enabled" : "Disabled"}</span>
			</div>
		{/if}
		<Field><FieldLabel>Type {app.name} to confirm</FieldLabel><Input bind:value={confirmationName} /></Field>
		<div class="flex justify-end gap-2">
			<Button type="button" variant="outline" onclick={() => (confirmationOpen = false)}>Cancel</Button>
			<Button type="button" disabled={confirmationName !== app.name || !pendingSync} onclick={() => pendingSync && startSync(pendingSync)}>Confirm sync</Button>
		</div>
	</DialogContent>
</Dialog>

{#snippet MissionBriefing()}
	<section class="overflow-hidden rounded-lg border bg-surface-1 shadow-sm">
		<div class="resource-tone-argo-surface p-3">
			<div class="flex flex-wrap items-start justify-between gap-3">
				<div class="min-w-0">
					<div class="flex flex-wrap items-center gap-1.5"><h2 class="truncate font-heading text-sm font-semibold">{resourceSummary.name}</h2><Badge variant="ghost"><CircleDot class="size-2.5 text-primary" /> Local demo</Badge></div>
					<div class="mt-2 flex flex-wrap items-center gap-2"><div class="font-heading text-2xl font-semibold tracking-tight">{session.healthStatus}</div><Badge variant="outline">{session.syncStatus}</Badge>{#if phaseLabel}<Badge variant="secondary" class="gap-1"><Spinner class="size-2.5" /> {phaseLabel}</Badge>{/if}</div>
					<div class="mt-1 max-w-xl text-xs leading-relaxed text-muted-foreground">{session.message}</div>
				</div>
				<div class="flex shrink-0 flex-wrap gap-1.5">{@render DashboardActions()}</div>
			</div>
		</div>
		<div class="grid grid-cols-3 gap-px bg-border">
			{@render SignalCard(counts.outOfSync, "Out of sync", "Needs reconciliation")}
			{@render SignalCard(counts.degraded + counts.progressing, "Attention", "Health is not settled")}
			{@render SignalCard(counts.prune, "Prune", "No longer in target")}
		</div>
	</section>

	{#each fixture.conditions as condition}
		<div class="flex items-start gap-2 rounded-lg border border-amber-500/25 bg-amber-500/5 p-3 text-xs">
			<TriangleAlert class="mt-0.5 size-3.5 shrink-0 text-amber-400" />
			<div><div class="font-medium">{condition.type}</div><div class="mt-0.5 leading-relaxed text-muted-foreground">{condition.message}</div></div>
		</div>
	{/each}

	{@render ChangeNavigator()}
	{@render MissionContextRow()}
{/snippet}

{#snippet ChangeNavigator()}
	<section class="rounded-lg border bg-surface-1 p-3">
		<div class="flex items-center justify-between gap-2"><div><div class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Change navigator</div><div class="mt-0.5 text-xs text-muted-foreground">Focus one reconciliation item or keep the complete stream.</div></div><Badge variant="outline">{attentionResources.length}</Badge></div>
		<div class="mt-3 flex gap-2 overflow-x-auto pb-1">
			<Button type="button" size="sm" variant={focusedResource ? "outline" : "secondary"} class="shrink-0" aria-pressed={!focusedResource} onclick={showAllChanges}>All changes</Button>
			{#each attentionResources as item}
				<Button type="button" size="sm" variant={focusedResource?.key === item.key ? "secondary" : "outline"} class="h-auto min-w-36 shrink-0 justify-start p-2 text-left" aria-pressed={focusedResource?.key === item.key} onclick={() => selectResource(item)}><span class="min-w-0"><span class="block truncate text-xs font-medium">{item.kind ?? "Resource"} · {item.name ?? "unknown"}</span><span class="mt-0.5 flex gap-1"><Badge variant="outline">{item.status ?? "Unknown"}</Badge>{#if item.requiresPruning}<Badge variant="destructive">Prune</Badge>{/if}</span></span></Button>
			{/each}
		</div>
	</section>
{/snippet}

{#snippet MissionContextRow()}
	<section class="grid gap-3 @min-[36rem]:grid-cols-[minmax(0,1.35fr)_minmax(12rem,0.65fr)]">
		{@render DeliveryCard()}
		{@render RecentDeploymentCard()}
	</section>
{/snippet}

{#snippet MissionDiffFeed()}
	<section class="rounded-lg border bg-surface-1 p-3">
		<div class="flex flex-wrap items-start justify-between gap-2">
			<div><div class="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground"><GitCompareArrows class="size-3.5" /> {focusedResource ? "Focused reconciliation" : "Continuous change stream"}</div><div class="mt-1 text-xs text-muted-foreground">{focusedResource ? resourceLabel(focusedResource) : `${diffResources.length} reconciliation items shown.`}</div></div>
			<div class="flex flex-wrap items-center gap-2">{#if focusedResource}<Button type="button" size="xs" variant="outline" aria-pressed={false} onclick={showAllChanges}>All changes</Button>{/if}<SegmentedControl value={visibleDiffView} options={diffViewOptions} onChange={(diffView) => patchArgoWorkspaceSession(sessionKey, { diffView })} ariaLabel="Reconciliation document view" size="sm" /></div>
		</div>
		<div class="mt-3 space-y-4">
			{#each diffResources as item}
				<article class="overflow-hidden rounded-lg border bg-background">
					<div class="flex flex-wrap items-center gap-2 border-b bg-surface-1 px-3 py-2"><div class="min-w-0 flex-1 truncate text-xs font-semibold">{resourceLabel(item)}</div>{#if item.requiresPruning}<Badge variant="destructive">Remove on sync</Badge>{:else if resourceAdded(item)}<Badge variant="outline" class="border-emerald-500/35 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300">Added</Badge>{:else if resourceModified(item)}<Badge variant="outline" class="border-amber-500/40 bg-amber-500/10 text-amber-700 dark:text-amber-300">Modified</Badge>{:else}<Badge variant="secondary">In sync</Badge>{/if}{#if visibleDiffView === "changes" && isRemovalOnlyResource(item)}<Button type="button" size="xs" variant="ghost" aria-expanded={removalDiffExpanded(item)} onclick={() => toggleRemovalDiff(item)}>{removalDiffExpanded(item) ? "Hide diff" : "Show diff"}</Button>{/if}{#if !focusedResource}<Button type="button" size="xs" variant="ghost" aria-pressed={false} onclick={() => selectResource(item)}>Focus</Button>{/if}</div>
					{#if visibleDiffView === "changes" && isRemovalOnlyResource(item)}
						<div class:border-b={removalDiffExpanded(item)} class="bg-destructive/10 p-3 font-sans text-xs leading-relaxed"><div class="font-medium text-destructive">Live-only resource</div><div class="mt-1 text-muted-foreground">This resource is no longer present in the desired revision. A sync with prune removes the live object shown below.</div></div>
					{/if}
					{#if visibleDiffView !== "changes" || !isRemovalOnlyResource(item) || removalDiffExpanded(item)}
						<div class="overflow-x-auto font-mono text-[0.6875rem] leading-5">
							{#if visibleDiffView === "changes" && isRemovalOnlyResource(item)}
								<div class="[&_.yaml-code-editor]:rounded-none [&_.yaml-code-editor]:border-0 [&_.yaml-code-editor]:shadow-none"><YamlCodeEditor value={resourceDocument(item, "live")} minHeight="180px" showErrorLens={false} /></div>
							{:else if visibleDiffView === "changes"}
								{#each resourceDiffLines(item) as line}<div class={`whitespace-pre px-3 ${diffLineClassName(line.type)}`}>{#each highlightedDiffSegments(line) as segment}<span class={segment.className}>{segment.text}</span>{/each}</div>{/each}
							{:else if visibleDiffView === "target" && item.requiresPruning}
								<div class="p-3 font-sans text-xs leading-relaxed"><div class="font-medium">No desired manifest</div><div class="mt-1 text-muted-foreground">The selected revision no longer contains this resource.</div></div>
							{:else}
								<div class="[&_.yaml-code-editor]:rounded-none [&_.yaml-code-editor]:border-0 [&_.yaml-code-editor]:shadow-none"><YamlCodeEditor value={resourceDocument(item, visibleDiffView)} minHeight="180px" showErrorLens={false} /></div>
							{/if}
						</div>
					{/if}
				</article>
			{:else}
				<Empty><EmptyHeader><EmptyTitle>No pending changes</EmptyTitle><EmptyDescription>The local prototype has no reconciliation items to display.</EmptyDescription></EmptyHeader></Empty>
			{/each}
		</div>
	</section>
{/snippet}

{#snippet DashboardActions()}
	<div class="inline-flex shrink-0">
		<Button type="button" size="sm" variant="outline" class="rounded-r-none" disabled={busy} onclick={() => refresh(false)}>{#if session.phase === "refreshing"}<Spinner />{:else}<RefreshCw data-icon="inline-start" />{/if}Refresh</Button>
		<Popover bind:open={refreshMenuOpen}>
			<PopoverTrigger type="button" class={buttonClass({ variant: "outline", size: "icon-sm", className: "rounded-l-none border-l-0" })} aria-label="Dashboard refresh options"><ChevronDown /></PopoverTrigger>
			<PopoverContent align="end" class="w-64 space-y-2"><div class="text-sm font-semibold">Refresh Application</div><div class="text-xs leading-relaxed text-muted-foreground">Normal refresh re-reads Application state. Hard refresh also invalidates Argo CD manifest and repository caches.</div><Button type="button" variant="outline" size="sm" class="w-full" disabled={busy} onclick={() => refresh(true)}><RotateCcw data-icon="inline-start" /> Hard refresh</Button></PopoverContent>
		</Popover>
	</div>
	<div class="inline-flex shrink-0">
		<Button type="button" size="sm" class="rounded-r-none" disabled={busy} onclick={() => startSync()}>{#if session.phase === "syncQueued" || session.phase === "syncing"}<Spinner />{:else}<RotateCcw data-icon="inline-start" />{/if}Sync</Button>
		<Popover bind:open={syncMenuOpen}>
			<PopoverTrigger type="button" class={buttonClass({ size: "icon-sm", className: "rounded-l-none border-l border-primary-foreground/20" })} aria-label="Dashboard sync options"><ChevronDown /></PopoverTrigger>
			<PopoverContent align="end" class="w-80 space-y-3">
				<div><div class="text-sm font-semibold">Sync with options</div><div class="mt-0.5 text-xs leading-relaxed text-muted-foreground">The main Sync button uses Application defaults. Advanced choices stay one level deeper.</div></div>
				<Field><FieldLabel>Revision override</FieldLabel><Input value={advancedSync.revision} placeholder={fixture.configuredRevision} oninput={(event: Event) => advancedSync = { ...advancedSync, revision: (event.currentTarget as HTMLInputElement).value }} /></Field>
				<div class="grid grid-cols-2 gap-2"><label class="flex items-center gap-2 rounded-md border p-2 text-xs"><Checkbox checked={advancedSync.prune} onCheckedChange={(checked) => advancedSync = { ...advancedSync, prune: checked }} />Prune</label><label class="flex items-center gap-2 rounded-md border p-2 text-xs"><Checkbox checked={advancedSync.dryRun} onCheckedChange={(checked) => advancedSync = { ...advancedSync, dryRun: checked }} />Dry run</label><label class="col-span-2 flex items-center gap-2 rounded-md border p-2 text-xs"><Checkbox checked={advancedSync.force} onCheckedChange={(checked) => advancedSync = { ...advancedSync, force: checked }} />Force / replace</label></div>
				<Button type="button" class="w-full" disabled={busy} onclick={requestAdvancedSync}>Sync with these options</Button>
			</PopoverContent>
		</Popover>
	</div>
{/snippet}

{#snippet DeliveryCard()}
	<div class="rounded-lg border bg-surface-1 p-3"><div class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Delivery</div><div class="mt-3 grid grid-cols-[4.5rem_1fr] gap-x-2 gap-y-2 text-xs"><span class="text-muted-foreground">Repository</span><span class="break-all">{fixture.repository}</span><span class="text-muted-foreground">Path</span><span>{fixture.path}</span><span class="text-muted-foreground">Destination</span><span>{fixture.destinationNamespace}</span><span class="text-muted-foreground">Policy</span><span>Automated · prune · self-heal</span></div></div>
{/snippet}

{#snippet RecentDeploymentCard()}
	<div class="rounded-lg border bg-surface-1 p-3"><div class="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground"><GitCommitHorizontal class="size-3.5" /> Recent deployment</div>{#if selectedHistory}<div class="mt-3 text-sm font-semibold">{historyRevision(selectedHistory)}</div><div class="mt-1 text-xs leading-relaxed text-muted-foreground">{selectedHistory.deployedAt} by {selectedHistory.initiatedBy ?? "unknown"}</div>{/if}</div>
{/snippet}

{#snippet SignalCard(value: number, label: string, description: string)}
	<div class="bg-surface-1 p-2.5"><div class="flex items-baseline gap-1.5"><div class="text-base font-semibold tabular-nums">{value}</div><div class="text-xs font-medium">{label}</div></div><div class="mt-0.5 text-[0.65rem] text-muted-foreground">{description}</div></div>
{/snippet}
