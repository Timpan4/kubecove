<script lang="ts">
	import {
		ArrowRight,
		GitBranch,
		PanelRightOpen,
		RefreshCw,
		RotateCcw,
		Wifi,
	} from "lucide-svelte";
	import CopyableText from "@/components/CopyableText.svelte";
	import TimestampText from "@/components/TimestampText.svelte";
	import { STATUS_BADGE_STYLES } from "@/components/status-badge-styles";
	import HealthAssessmentBadge from "@/components/HealthAssessmentBadge.svelte";
	import { Badge, Button, Spinner } from "@/components/ui/svelte";
	import { healthStatusVariant, syncStatusVariant, type ChipVariant } from "@/features/argo/status";
	import type {
		ArgoApplicationInspector,
		ArgoApplicationSummary,
		ArgoManagedResource,
	} from "@/lib/gitops-types";
	import type { JsonObject, JsonValue } from "@/lib/types";
	import { argoResourceCounts } from "./argo-workspace-model";

	let {
		app,
		inspector = null,
		managedResources = [],
		loading = false,
		refreshing = false,
		refreshDisabled = false,
		error = null,
		onRefresh = async () => {},
		onInspect = () => {},
	}: {
		app: ArgoApplicationSummary;
		inspector?: ArgoApplicationInspector | null;
		managedResources?: ArgoManagedResource[];
		loading?: boolean;
		refreshing?: boolean;
		refreshDisabled?: boolean;
		error?: unknown;
		onRefresh?: () => Promise<void>;
		onInspect?: (app: ArgoApplicationSummary) => void;
	} = $props();

	const primarySource = $derived(app.sources?.[0] ?? null);
	const repository = $derived(primarySource?.repoUrl ?? app.sourceRepo ?? "Repository unavailable");
	const sourceDetail = $derived(
		primarySource?.path ?? primarySource?.chart ?? app.sourceMode ?? "Source path unavailable",
	);
	const configuredRevision = $derived(
		primarySource?.targetRevision ?? app.sourceRevision ?? "Revision unavailable",
	);
	const resolvedRevision = $derived(
		statusValue(inspector?.status, "sync", "revision") ??
			primarySource?.resolvedRevision ??
			inspector?.history[0]?.revision ??
			configuredRevision,
	);
	const destination = $derived(app.destinationNamespace ?? app.destinationServer ?? "Target unavailable");
	const destinationDetail = $derived(
		app.destinationNamespace && app.destinationServer
			? app.destinationServer
			: `${app.project ?? "default"} project`,
	);
	const healthStatus = $derived(
		statusValue(inspector?.status, "health", "status") ?? app.healthStatus ?? "Unknown",
	);
	const syncStatus = $derived(
		statusValue(inspector?.status, "sync", "status") ?? app.syncStatus ?? "Unknown",
	);
	const reconciledAt = $derived(statusValue(inspector?.status, "reconciledAt"));
	const resourceCounts = $derived(argoResourceCounts(managedResources));
	const healthTone = $derived(healthStatusVariant(healthStatus));
	const syncTone = $derived(syncStatusVariant(syncStatus));
	const errorMessage = $derived(
		error instanceof Error ? error.message : error ? String(error) : null,
	);
	const connectionMessage = $derived(
		loading
			? "Loading Application and managed resources…"
			: errorMessage
				? `Argo data unavailable: ${errorMessage}`
				: inspector?.connectedFallback
						? `Kubernetes fallback after Connected ${inspector.connectedFallback.failure.kind}: ${inspector.connectedFallback.failure.message}`
						: reconciledAt
							? null
							: `${inspector?.connected ? "Argo CD API" : "Application CRD"} managed-resource state`,
	);

	function statusBadgeVariant(tone: ChipVariant) {
		return STATUS_BADGE_STYLES[tone].variant;
	}

	function statusBadgeClass(tone: ChipVariant) {
		return `rounded-full px-2 py-0 text-[0.6875rem] shadow-none ${STATUS_BADGE_STYLES[tone].className}`;
	}

	function statusValue(value: JsonValue | undefined, ...path: string[]): string | null {
		let current: JsonValue | undefined = value;
		for (const key of path) {
			if (!isRecord(current)) return null;
			current = current[key];
		}
		return isString(current) && current.trim() ? current : null;
	}

	function isRecord(value: JsonValue | undefined): value is JsonObject {
		return value !== null && !Array.isArray(value) && Object(value) === value;
	}

	function isString(value: JsonValue | undefined): value is string {
		return String(value) === value;
	}
</script>

<section class="min-w-0 overflow-hidden rounded-lg border border-sidebar-border bg-surface-1 shadow-sm">
	<div class="flex min-w-0 flex-wrap items-start gap-3 border-b px-3 py-2.5">
		<div class="flex min-w-0 flex-1 items-center gap-2.5">
			<div class="grid size-8 shrink-0 place-items-center rounded-md resource-tone-argo-surface">
				<GitBranch class="size-4 text-[var(--resource-argo)]" />
			</div>
			<div class="min-w-0">
				<div class="flex min-w-0 flex-wrap items-center gap-1.5">
					<h2 class="min-w-0 flex-1 font-heading text-sm font-semibold">
						<CopyableText value={app.name} label="application name" />
					</h2>
					<HealthAssessmentBadge assessment={app.healthAssessment} {loading} />
				</div>
				<div class="mt-1 flex min-w-0 flex-wrap items-center gap-1.5">
					<span class="text-[0.6875rem] text-muted-foreground">Raw Argo:</span>
					<Badge variant={statusBadgeVariant(healthTone)} class={statusBadgeClass(healthTone)}>{healthStatus}</Badge>
					<Badge variant={statusBadgeVariant(syncTone)} class={statusBadgeClass(syncTone)}>{syncStatus}</Badge>
				</div>
				<div class="mt-0.5 flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground" role="status" aria-live="polite">
					<Wifi class={`size-3 shrink-0 ${errorMessage ? "text-red-400" : "text-emerald-400"}`} />
					{#if reconciledAt && !connectionMessage}
						<span class="truncate">Reconciled <TimestampText value={reconciledAt} precision="millisecond" /></span>
					{:else}
						<span class="truncate">{connectionMessage}</span>
					{/if}
				</div>
			</div>
		</div>
		<div class="max-w-full shrink-0 overflow-x-auto">
			<div class="flex w-max items-center gap-1.5">
				<Button type="button" size="sm" variant="outline" disabled={loading || refreshing || refreshDisabled} onclick={() => void onRefresh()}>
					{#if refreshing}<Spinner />{:else}<RefreshCw data-icon="inline-start" />{/if}
					Refresh
				</Button>
				<Button type="button" size="sm" disabled aria-describedby="argo-overview-sync-gap">
					<RotateCcw data-icon="inline-start" /> Sync
				</Button>
				<Button type="button" size="sm" variant="ghost" onclick={() => onInspect(app)}>
					<PanelRightOpen data-icon="inline-start" /> Dashboard
				</Button>
			</div>
		</div>
		<p id="argo-overview-sync-gap" class="w-full text-right text-[0.6875rem] text-muted-foreground">
			Sync unavailable here until operation progress tracking is wired to this overview.
		</p>
	</div>

	<div class="overflow-x-auto">
		<div class="resource-tone-argo-surface grid min-w-[70rem] grid-cols-[minmax(16rem,1fr)_auto_minmax(9rem,0.55fr)_auto_minmax(11rem,0.65fr)_auto_minmax(24rem,1.2fr)] items-center gap-2 p-2.5">
			{@render RouteCard("1", "Repository", repository, sourceDetail)}
			<ArrowRight class="size-4 text-muted-foreground" />
			{@render RouteCard("2", "Release", resolvedRevision, `from ${configuredRevision}`)}
			<ArrowRight class="size-4 text-muted-foreground" />
			{@render RouteCard("3", "Cluster target", destination, destinationDetail)}
			<ArrowRight class="size-4 text-muted-foreground" />
			<div class="min-w-0 rounded-md border border-sidebar-border/70 bg-surface-1/50 p-2.5">
				<div class="flex items-center gap-2">
					<span class="grid size-5 shrink-0 place-items-center rounded-full bg-amber-400/15 text-[0.625rem] font-semibold text-amber-300">4</span>
					<div class="text-[0.625rem] font-medium uppercase tracking-wide text-muted-foreground">Resource result</div>
					<div class="ml-auto text-xs">
						<strong class="font-semibold tabular-nums">{resourceCounts.needsSync}</strong> need sync ·
						<strong class="font-semibold tabular-nums">{resourceCounts.current}</strong> current ·
						<strong class="font-semibold tabular-nums">{resourceCounts.total}</strong> total
					</div>
				</div>
				<div class="mt-1.5 flex h-1.5 overflow-hidden rounded-full bg-border/50" role="img" aria-label={`${resourceCounts.needsSync} resources need sync plus ${resourceCounts.current} current resources equals ${resourceCounts.total} managed resources`}>
					{#if resourceCounts.needsSync}<span class="basis-0 bg-amber-400/70" style={`flex-grow: ${resourceCounts.needsSync}`}></span>{/if}
					{#if resourceCounts.current}<span class="basis-0 bg-emerald-400/70" style={`flex-grow: ${resourceCounts.current}`}></span>{/if}
				</div>
				<div class="mt-1.5 flex min-w-0 items-center gap-2">
					<span class="shrink-0 text-[0.625rem] text-muted-foreground">Health and prune may overlap sync state</span>
					<div class="flex min-w-0 flex-wrap gap-1">
						{#if resourceCounts.degraded}<Badge variant={statusBadgeVariant("error")} class={statusBadgeClass("error")}>{resourceCounts.degraded} degraded</Badge>{/if}
						{#if resourceCounts.progressing}<Badge variant={statusBadgeVariant("warning")} class={statusBadgeClass("warning")}>{resourceCounts.progressing} progressing</Badge>{/if}
						{#if resourceCounts.prune}<Badge variant={statusBadgeVariant("error")} class={statusBadgeClass("error")}>{resourceCounts.prune} to prune</Badge>{/if}
						{#if !resourceCounts.degraded && !resourceCounts.progressing && !resourceCounts.prune}<Badge variant={statusBadgeVariant("success")} class={statusBadgeClass("success")}>No additional signals</Badge>{/if}
					</div>
				</div>
			</div>
		</div>
	</div>
</section>

{#snippet RouteCard(step: string, label: string, value: string, detail: string)}
	<div class="min-w-0 rounded-md border border-sidebar-border/70 bg-surface-1/50 p-2.5">
		<div class="flex items-center gap-2">
			<span class="grid size-5 shrink-0 place-items-center rounded-full resource-tone-argo-surface text-[0.625rem] font-semibold text-[var(--resource-argo)]">{step}</span>
			<div class="text-[0.625rem] font-medium uppercase tracking-wide text-muted-foreground">{label}</div>
		</div>
		<CopyableText class="mt-1" textClass="text-sm font-semibold" {value} label={`${label.toLocaleLowerCase()} value`} />
		<CopyableText textClass="text-[0.6875rem] text-muted-foreground" value={detail} label={`${label.toLocaleLowerCase()} detail`} />
	</div>
{/snippet}
