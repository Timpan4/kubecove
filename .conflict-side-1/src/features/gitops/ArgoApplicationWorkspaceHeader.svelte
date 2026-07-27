<script lang="ts">
	import {
		ArrowRight,
		ChevronDown,
		GitBranch,
		GitCompareArrows,
		PanelRightOpen,
		RefreshCw,
		RotateCcw,
		Wifi,
	} from "lucide-svelte";
	import PrototypeSwitcher, { type PrototypeVariantOption } from "@/components/PrototypeSwitcher.svelte";
	import { STATUS_BADGE_STYLES } from "@/components/status-badge-styles";
	import {
		Badge,
		Button,
		Checkbox,
		Dialog,
		DialogContent,
		DialogDescription,
		DialogHeader,
		DialogTitle,
		Field,
		FieldLabel,
		Input,
		Popover,
		PopoverContent,
		PopoverTrigger,
		Spinner,
		buttonClass,
	} from "@/components/ui/svelte";
	import { healthStatusVariant, syncStatusVariant, type ChipVariant } from "@/features/argo/status";
	import { showToast } from "@/lib/toasts";
	import type { ArgoApplicationSummary } from "@/lib/gitops-types";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import {
		ARGO_SYNC_DEFAULTS,
		argoPhaseLabel,
		argoPrototypeResources,
		argoResourceCounts,
		createArgoPrototypeFixture,
		needsArgoSyncConfirmation,
		type ArgoSyncSettings,
	} from "./argo-workspace-model";
	import {
		argoWorkspaceSessionKey,
		argoWorkspaceSessions,
		createArgoWorkspaceSession,
		ensureArgoWorkspaceSession,
		patchArgoWorkspaceSession,
		type ArgoHeaderVariation,
	} from "./argo-workspace-session";

	let {
		app,
		workspaceReadContext,
		onInspect = () => {},
	}: {
		app: ArgoApplicationSummary;
		workspaceReadContext: WorkspaceReadContext;
		onInspect?: (app: ArgoApplicationSummary) => void;
	} = $props();

	const variationOptions: PrototypeVariantOption[] = [
		{ value: "briefing", label: "Briefing bar", description: "Details-pane rhythm in two rows" },
		{ value: "signals", label: "Signal deck", description: "Health and reconciliation lead" },
		{ value: "flow", label: "Delivery flow", description: "Source-to-cluster movement leads" },
	];
	const prototypeQueryKey = "argoOverviewPrototype";
	const sessionKey = $derived(
		argoWorkspaceSessionKey(
			workspaceReadContext.clusterContext,
			workspaceReadContext.workspaceId,
			app.namespace,
			app.name,
		),
	);
	const fixture = $derived(createArgoPrototypeFixture(app));
	const session = $derived($argoWorkspaceSessions[sessionKey] ?? createArgoWorkspaceSession());
	const resources = $derived(argoPrototypeResources(fixture, session.synced));
	const resourceCounts = $derived(argoResourceCounts(resources));
	const phaseLabel = $derived(argoPhaseLabel(session.phase));
	const healthTone = $derived(healthStatusVariant(session.healthStatus));
	const syncTone = $derived(syncStatusVariant(session.syncStatus));
	const busy = $derived(session.phase !== "idle");

	let refreshMenuOpen = $state(false);
	let syncMenuOpen = $state(false);
	let advancedSync = $state<ArgoSyncSettings>({ ...ARGO_SYNC_DEFAULTS });
	let confirmationOpen = $state(false);
	let confirmationName = $state("");
	let pendingSync = $state<ArgoSyncSettings | null>(null);

	$effect(() => {
		ensureArgoWorkspaceSession(sessionKey);
		if (typeof window === "undefined") return;
		const requested = new URL(window.location.href).searchParams.get(prototypeQueryKey);
		if (
			(requested === "briefing" || requested === "signals" || requested === "flow") &&
			requested !== session.headerVariation
		) {
			patchArgoWorkspaceSession(sessionKey, { headerVariation: requested });
		}
	});

	function statusBadgeVariant(tone: ChipVariant) {
		return STATUS_BADGE_STYLES[tone].variant;
	}

	function statusBadgeClass(tone: ChipVariant) {
		return `rounded-full px-2 py-0 text-[0.6875rem] shadow-none ${STATUS_BADGE_STYLES[tone].className}`;
	}

	function setVariation(value: ArgoHeaderVariation) {
		patchArgoWorkspaceSession(sessionKey, { headerVariation: value });
		if (typeof window === "undefined") return;
		const url = new URL(window.location.href);
		url.searchParams.set(prototypeQueryKey, value);
		window.history.replaceState(window.history.state, "", url);
	}

	function openDashboard(tab: "overview" | "resources" | "history" | "diff") {
		patchArgoWorkspaceSession(sessionKey, { dashboardTab: tab });
		onInspect(app);
	}

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
			message: settings.dryRun ? "Dry run queued with the selected options…" : "Sync queued with the selected options…",
			syncDraft: { ...settings },
		});
		setTimeout(() => {
			patchArgoWorkspaceSession(sessionKey, {
				phase: "syncing",
				healthStatus: settings.dryRun ? session.healthStatus : "Progressing",
				message: settings.dryRun ? "Rendering and validating manifests…" : "Applying target revision to managed resources…",
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
</script>

<div>
{#if session.headerVariation === "briefing"}
	{@render OverviewBriefing()}
{:else if session.headerVariation === "signals"}
	{@render OverviewSignals()}
{:else}
	{@render OverviewFlow()}
{/if}
</div>

<PrototypeSwitcher label="Resource overview" variants={variationOptions} value={session.headerVariation} onChange={(value) => setVariation(value as ArgoHeaderVariation)} />

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

{#snippet OverviewBriefing()}
	<section class="overflow-hidden rounded-lg border border-sidebar-border bg-surface-1 shadow-sm">
		<div class="resource-tone-argo-surface flex min-w-0 items-center gap-3 border-b px-3 py-2.5">
			<div class="flex min-w-0 flex-1 items-center gap-2.5">
				{@render ApplicationMark()}
				<div class="min-w-0">
					<div class="flex min-w-0 items-center gap-1.5"><h2 class="truncate font-heading text-sm font-semibold">{app.name}</h2>{@render StatusBadges()}</div>
					{@render ConnectionMessage()}
				</div>
			</div>
			<div class="flex shrink-0 items-center gap-1.5 overflow-x-auto">{@render ActionControls()}</div>
		</div>
		<div class="overflow-x-auto bg-border">
			<div class="grid min-w-[46rem] grid-cols-[1.2fr_0.8fr_0.8fr_1fr] gap-px">
				{@render SourceFact()}
				{@render DestinationFact()}
				{@render ApplicationFact()}
				{@render ResourceFact()}
			</div>
		</div>
	</section>
{/snippet}

{#snippet OverviewSignals()}
	<section class="overflow-x-auto rounded-lg border border-sidebar-border bg-surface-1 shadow-sm">
		<div class="grid min-w-[50rem] grid-cols-[12rem_minmax(18rem,1fr)_19rem]">
			<div class="resource-tone-argo-surface border-r p-3">
				<div class="flex items-center gap-2">{@render ApplicationMark()}<h2 class="truncate font-heading text-sm font-semibold">{app.name}</h2></div>
				<div class="mt-2 flex items-end justify-between gap-2"><div><div class="text-[0.625rem] font-semibold uppercase tracking-[0.14em] text-muted-foreground">Health</div><div class="mt-0.5 font-heading text-xl font-semibold tracking-tight">{session.healthStatus}</div></div><Badge variant={statusBadgeVariant(syncTone)} class={statusBadgeClass(syncTone)}>{session.syncStatus}</Badge></div>
			</div>
			<div class="min-w-0 p-3">
				<div class="flex min-w-0 items-center gap-2">{#if phaseLabel}<Badge variant="secondary" class="shrink-0 gap-1"><Spinner class="size-2.5" /> {phaseLabel}</Badge>{/if}<div class="truncate text-xs font-medium">{session.message}</div></div>
				<div class="mt-2 grid grid-cols-[minmax(0,1fr)_auto_auto] gap-4 text-xs"><div class="min-w-0"><div class="text-[0.625rem] uppercase tracking-wide text-muted-foreground">Source</div><div class="truncate font-medium">{fixture.repository}</div></div><div><div class="text-[0.625rem] uppercase tracking-wide text-muted-foreground">Destination</div><div class="font-medium">{fixture.destinationNamespace}</div></div><div><div class="text-[0.625rem] uppercase tracking-wide text-muted-foreground">Revision</div><div class="font-medium">{fixture.resolvedRevision}</div></div></div>
			</div>
			<div class="border-l bg-surface-0 p-2.5">
				<div class="grid grid-cols-3 gap-px overflow-hidden rounded-md border bg-border">{@render SignalCell(resourceCounts.outOfSync, "Drift", "out of sync")}{@render SignalCell(resourceCounts.degraded + resourceCounts.progressing, "Attention", "health")}{@render SignalCell(resourceCounts.prune, "Prune", "on sync")}</div>
				<div class="mt-2 flex justify-end gap-1.5">{@render ActionControls()}</div>
			</div>
		</div>
	</section>
{/snippet}

{#snippet OverviewFlow()}
	<section class="overflow-hidden rounded-lg border border-sidebar-border bg-surface-1 shadow-sm">
		<div class="flex min-w-0 items-center gap-3 border-b px-3 py-2.5">
			<div class="flex min-w-0 flex-1 items-center gap-2.5">{@render ApplicationMark()}<div class="min-w-0"><div class="flex min-w-0 items-center gap-1.5"><h2 class="truncate font-heading text-sm font-semibold">{app.name}</h2>{@render StatusBadges()}</div>{@render ConnectionMessage()}</div></div>
			<div class="flex shrink-0 items-center gap-1.5 overflow-x-auto">{@render ActionControls()}</div>
		</div>
		<div class="overflow-x-auto">
			<div class="resource-tone-argo-surface flex min-w-[48rem] items-center gap-3 px-3 py-2">
				{@render FlowFact("Source", fixture.repository, fixture.configuredRevision)}<ArrowRight class="size-3.5 shrink-0 text-muted-foreground" />{@render FlowFact("Revision", fixture.resolvedRevision, fixture.configuredRevision)}<ArrowRight class="size-3.5 shrink-0 text-muted-foreground" />{@render FlowFact("Destination", fixture.destinationNamespace, app.project ?? "default")}
				<div class="ml-auto flex shrink-0 items-center gap-3"><div><div class="text-sm font-semibold tabular-nums">{resourceCounts.total}</div><div class="text-[0.625rem] text-muted-foreground">resources</div></div><div class="flex flex-wrap gap-1">{@render ResourceBadges()}</div></div>
			</div>
		</div>
	</section>
{/snippet}

{#snippet SignalCell(value: number, label: string, description: string)}
	<div class="bg-surface-1 px-2 py-1.5 text-center"><div class="text-sm font-semibold tabular-nums">{value}</div><div class="text-[0.625rem] font-medium">{label}</div><div class="text-[0.5625rem] text-muted-foreground">{description}</div></div>
{/snippet}

{#snippet FlowFact(label: string, value: string, detail: string)}
	<div class="min-w-0 flex-1"><div class="text-[0.625rem] font-medium uppercase tracking-wide text-muted-foreground">{label}</div><div class="truncate text-xs font-medium">{value}</div><div class="truncate text-[0.625rem] text-muted-foreground">{detail}</div></div>
{/snippet}

{#snippet ApplicationMark()}
	<div class="grid size-8 shrink-0 place-items-center rounded-md resource-tone-argo-surface"><GitBranch class="size-4 text-[var(--resource-argo)]" /></div>
{/snippet}

{#snippet StatusBadges()}
	<Badge variant={statusBadgeVariant(healthTone)} class={statusBadgeClass(healthTone)}>{session.healthStatus}</Badge>
	<Badge variant={statusBadgeVariant(syncTone)} class={statusBadgeClass(syncTone)}>{session.syncStatus}</Badge>
	{#if phaseLabel}<Badge variant="secondary" class="shrink-0 gap-1"><Spinner class="size-2.5" /> {phaseLabel}</Badge>{/if}
{/snippet}

{#snippet ConnectionMessage()}
	<div class="mt-0.5 flex min-w-0 items-center gap-1.5 truncate text-xs text-muted-foreground"><Wifi class="size-3 shrink-0 text-emerald-400" /><span class="shrink-0">Connected Argo CD</span><span aria-hidden="true">·</span><span class="truncate">{session.message}</span></div>
{/snippet}

{#snippet ActionControls()}
	<div class="inline-flex shrink-0">
		<Button type="button" size="sm" variant="outline" class="rounded-r-none" disabled={busy} onclick={() => refresh(false)}>{#if session.phase === "refreshing"}<Spinner />{:else}<RefreshCw data-icon="inline-start" />{/if}Refresh</Button>
		<Popover bind:open={refreshMenuOpen}>
			<PopoverTrigger type="button" class={buttonClass({ variant: "outline", size: "icon-sm", className: "rounded-l-none border-l-0" })} aria-label="Refresh options"><ChevronDown /></PopoverTrigger>
			<PopoverContent align="end" class="w-64 space-y-2"><div class="text-sm font-semibold">Refresh Application</div><div class="text-xs leading-relaxed text-muted-foreground">Normal refresh re-reads Application state. Hard refresh also invalidates Argo CD manifest and repository caches.</div><Button type="button" variant="outline" size="sm" class="w-full" disabled={busy} onclick={() => refresh(true)}><RotateCcw data-icon="inline-start" /> Hard refresh</Button></PopoverContent>
		</Popover>
	</div>
	<div class="inline-flex shrink-0">
		<Button type="button" size="sm" class="rounded-r-none" disabled={busy} onclick={() => startSync()}>{#if session.phase === "syncQueued" || session.phase === "syncing"}<Spinner />{:else}<RotateCcw data-icon="inline-start" />{/if}Sync</Button>
		<Popover bind:open={syncMenuOpen}>
			<PopoverTrigger type="button" class={buttonClass({ size: "icon-sm", className: "rounded-l-none border-l border-primary-foreground/20" })} aria-label="Sync options"><ChevronDown /></PopoverTrigger>
			<PopoverContent align="end" class="w-80 space-y-3">
				<div><div class="text-sm font-semibold">Sync with options</div><div class="mt-0.5 text-xs leading-relaxed text-muted-foreground">The main Sync button uses Application defaults. Advanced choices stay one level deeper.</div></div>
				<Field><FieldLabel>Revision override</FieldLabel><Input value={advancedSync.revision} placeholder={fixture.configuredRevision} oninput={(event: Event) => advancedSync = { ...advancedSync, revision: (event.currentTarget as HTMLInputElement).value }} /></Field>
				<div class="grid grid-cols-2 gap-2"><label class="flex items-center gap-2 rounded-md border p-2 text-xs"><Checkbox checked={advancedSync.prune} onCheckedChange={(checked) => advancedSync = { ...advancedSync, prune: checked }} />Prune</label><label class="flex items-center gap-2 rounded-md border p-2 text-xs"><Checkbox checked={advancedSync.dryRun} onCheckedChange={(checked) => advancedSync = { ...advancedSync, dryRun: checked }} />Dry run</label><label class="col-span-2 flex items-center gap-2 rounded-md border p-2 text-xs"><Checkbox checked={advancedSync.force} onCheckedChange={(checked) => advancedSync = { ...advancedSync, force: checked }} />Force / replace</label></div>
				<Button type="button" class="w-full" disabled={busy} onclick={requestAdvancedSync}>Sync with these options</Button>
			</PopoverContent>
		</Popover>
	</div>
	<Button type="button" size="sm" variant="outline" class="shrink-0" onclick={() => openDashboard("diff")}><GitCompareArrows data-icon="inline-start" /> Diff</Button>
	<Button type="button" size="sm" variant="ghost" class="shrink-0" onclick={() => openDashboard("overview")}><PanelRightOpen data-icon="inline-start" /> Dashboard</Button>
{/snippet}

{#snippet SourceFact()}
	<div class="min-w-0 bg-surface-1 px-3 py-2"><div class="text-[0.625rem] font-medium uppercase tracking-wide text-muted-foreground">Source</div><div class="mt-0.5 truncate text-xs font-medium" title={fixture.repository}>{fixture.repository}</div><div class="truncate text-[0.6875rem] text-muted-foreground">{fixture.configuredRevision} · {fixture.resolvedRevision}</div></div>
{/snippet}

{#snippet DestinationFact()}
	<div class="min-w-0 bg-surface-1 px-3 py-2"><div class="text-[0.625rem] font-medium uppercase tracking-wide text-muted-foreground">Destination</div><div class="mt-0.5 truncate text-xs font-medium">{fixture.destinationNamespace}</div><div class="truncate text-[0.6875rem] text-muted-foreground">{app.project ?? "default"} project</div></div>
{/snippet}

{#snippet ApplicationFact()}
	<div class="min-w-0 bg-surface-1 px-3 py-2"><div class="text-[0.625rem] font-medium uppercase tracking-wide text-muted-foreground">Application</div><div class="mt-0.5 truncate text-xs font-medium">{app.namespace ?? "argocd"} namespace</div><div class="truncate text-[0.6875rem] text-muted-foreground">Created {app.age || "recently"}</div></div>
{/snippet}

{#snippet ResourceFact()}
	<div class="flex min-w-0 items-center gap-3 bg-surface-1 px-3 py-2"><div><div class="text-sm font-semibold tabular-nums">{resourceCounts.total}</div><div class="text-[0.625rem] text-muted-foreground">resources</div></div><div class="flex min-w-0 flex-wrap gap-1">{@render ResourceBadges()}</div></div>
{/snippet}

{#snippet ResourceBadges()}
	{#if resourceCounts.outOfSync}<Badge variant={statusBadgeVariant("warning")} class={statusBadgeClass("warning")}>{resourceCounts.outOfSync} out of sync</Badge>{/if}
	{#if resourceCounts.degraded}<Badge variant={statusBadgeVariant("error")} class={statusBadgeClass("error")}>{resourceCounts.degraded} degraded</Badge>{/if}
	{#if resourceCounts.progressing}<Badge variant={statusBadgeVariant("warning")} class={statusBadgeClass("warning")}>{resourceCounts.progressing} progressing</Badge>{/if}
	{#if resourceCounts.prune}<Badge variant={statusBadgeVariant("error")} class={statusBadgeClass("error")}>{resourceCounts.prune} prune</Badge>{/if}
	{#if session.synced}<Badge variant={statusBadgeVariant("success")} class={statusBadgeClass("success")}>All current</Badge>{/if}
{/snippet}
