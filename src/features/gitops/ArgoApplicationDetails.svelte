<script lang="ts">
	import { yamlLanguage } from "@codemirror/lang-yaml";
	import { highlightTree, tagHighlighter, tags } from "@lezer/highlight";
	import { createQueries, createQuery, useQueryClient } from "@tanstack/svelte-query";
	import {
		ChevronDown,
		CircleDot,
		GitCompareArrows,
		GitCommitHorizontal,
		RefreshCw,
		RotateCcw,
		TriangleAlert,
	} from "lucide-svelte";
	import { parse, stringify } from "yaml";
	import ArgoConnectionSettings from "@/app/svelte/ArgoConnectionSettings.svelte";
	import YamlCodeEditor from "@/components/YamlCodeEditor.svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
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
		Select,
		SelectContent,
		SelectGroup,
		SelectItem,
		SelectTrigger,
		SelectValue,
		Spinner,
		buttonClass,
	} from "@/components/ui/svelte";
	import {
		buildArgoResourceDiff,
		diffLineClassName,
		type UnifiedDiffLine,
	} from "@/features/resource-detail/yamlTabDiff";
	import { queryKeys } from "@/lib/queryKeys";
	import { settingsStore } from "@/lib/settings-store";
	import {
		createTauriClient,
		getArgoApplicationInspector,
		getArgoApplicationResources,
		getArgoConnectionStatus,
		getArgoResourceComparison,
		preflightArgoOperation,
		runArgoOperation,
	} from "@/lib/tauri";
	import type {
		ArgoApplicationHistory,
		ArgoApplicationRef,
		ArgoApplicationSummary,
		ArgoManagedResource,
		ArgoOperationAction,
		ArgoOperationRequest,
		ArgoResourceComparison,
		ResourceDetailsFull,
		ResourceSummary,
	} from "@/lib/types";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import {
		ArgoOperationRefreshError,
		runArgoOperationLifecycle,
	} from "./argo-operation-lifecycle";
	import {
		applyArgoSyncDefaults,
		argoComparisonDocument,
		argoHistoryKey,
		argoReconciliationResources,
		argoResourceCounts,
		argoResourceIdentityKey,
		argoSyncNeedsConfirmation,
		defaultArgoSyncSettings,
		preserveArgoHistorySelection,
		preserveArgoResourceSelection,
		withArgoSyncSettings,
		type ArgoSyncSettings,
	} from "./argo-workspace-model";

	let {
		resourceSummary,
		applicationSummary = null,
		applicationDetails = null,
		workspaceReadContext,
		active,
	}: {
		resourceSummary: ResourceSummary;
		applicationSummary?: ArgoApplicationSummary | null;
		applicationDetails?: ResourceDetailsFull | null;
		workspaceReadContext: WorkspaceReadContext;
		active: boolean;
	} = $props();

	type DiffView = "changes" | "target" | "live";
	type OperationPhase = "idle" | "authorizing" | "submitting" | "refreshing" | "accepted" | "error";
	type YamlDiffSegment = { text: string; className: string };

	const client = createTauriClient();
	const queryClient = useQueryClient();
	const diffViewOptions: { value: DiffView; label: string }[] = [
		{ value: "changes", label: "Changes" },
		{ value: "target", label: "Desired" },
		{ value: "live", label: "Live" },
	];
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

	const clusterContext = $derived(workspaceReadContext.clusterContext);
	const workspaceId = $derived(workspaceReadContext.workspaceId);
	const kubeconfigEnvVar = $derived(workspaceReadContext.kubeconfigSourceKey);
	const redactSecrets = $derived($settingsStore.redactSecrets);
	const matchingSummary = $derived(
		applicationSummary &&
			applicationSummary.cluster === clusterContext &&
			applicationSummary.name === resourceSummary.name &&
			(applicationSummary.namespace ?? "") === (resourceSummary.namespace ?? "")
			? applicationSummary
			: null,
	);
	const applicationRequest = $derived<ArgoApplicationRef>({
		name: resourceSummary.name,
		namespace: resourceSummary.namespace,
		project: matchingSummary?.project ?? null,
		resourceVersion: matchingSummary?.resourceVersion ?? null,
		uid: matchingSummary?.uid ?? null,
		apiVersion: resourceSummary.apiVersion ?? "argoproj.io/v1alpha1",
		context: clusterContext,
		workspaceId,
	});
	const matchingProfiles = $derived(
		$settingsStore.argoProfiles.filter(
			(profile) =>
				profile.clusterContext === clusterContext &&
				(!profile.workspaceId || profile.workspaceId === workspaceId),
		),
	);

	let transport = $state<"connected" | "kubernetes">("kubernetes");
	let connectionId = $state("");
	let selectedResource = $state<ArgoManagedResource | null>(null);
	let comparisonSlots = $state.raw<(ArgoManagedResource | null)[]>([]);
	let selectedHistoryKey = $state<string | null>(null);
	let diffView = $state<DiffView>("changes");
	let expandedRemovalKeys = $state<string[]>([]);
	let connectionSettingsOpen = $state(false);
	let refreshMenuOpen = $state(false);
	let syncMenuOpen = $state(false);
	let advancedSync = $state<ArgoSyncSettings>({ ...defaultArgoSyncSettings });
	let appliedSyncDefaults = $state<ArgoSyncSettings>({ ...defaultArgoSyncSettings });
	let appliedSyncDefaultsKey = $state("");
	let confirmationOpen = $state(false);
	let confirmationName = $state("");
	let pendingSync = $state<ArgoSyncSettings | null>(null);
	let operationPhase = $state<OperationPhase>("idle");
	let operationMessage = $state<string | null>(null);
	let operationError = $state<string | null>(null);
	let acceptedRefreshPending = $state(false);
	let lastOperationRequest = $state<ArgoOperationRequest | null>(null);
	let appliedScopeKey = $state("");

	const statuses = createQuery(() => ({
		queryKey: [
			"argo-connection-status",
			clusterContext,
			workspaceId,
			matchingProfiles.map((profile) => profile.id).join(","),
		],
		queryFn: () =>
			Promise.all(
				matchingProfiles.map(
					async (profile) => [profile.id, await getArgoConnectionStatus(client, profile.id)] as const,
				),
			),
		enabled: active && matchingProfiles.length > 0,
		staleTime: 5_000,
	}));
	const selectedStatus = $derived(statuses.data?.find(([id]) => id === connectionId)?.[1]);
	const selectedProfile = $derived(matchingProfiles.find((profile) => profile.id === connectionId) ?? null);
	const request = $derived({
		clusterContext,
		kubeconfigEnvVar,
		connectionId: transport === "connected" ? connectionId : undefined,
		transport,
		application: applicationRequest,
		redactSecrets,
	});
	const inspector = createQuery(() => ({
		queryKey: queryKeys.argoWorkspaceInspector(
			clusterContext,
			workspaceId,
			resourceSummary.name,
			resourceSummary.namespace,
			applicationRequest.uid,
			redactSecrets,
			transport,
			connectionId,
			kubeconfigEnvVar,
		),
		queryFn: () => getArgoApplicationInspector(client, request),
		enabled:
			active &&
			workspaceReadContext.sourceReady &&
			(transport === "kubernetes" || Boolean(connectionId)),
		staleTime: 30_000,
		retry: false,
		gcTime: redactSecrets ? undefined : 0,
	}));
	const resources = createQuery(() => ({
		queryKey: queryKeys.argoWorkspaceManagedResources(
			clusterContext,
			workspaceId,
			resourceSummary.name,
			resourceSummary.namespace,
			applicationRequest.uid,
			redactSecrets,
			transport,
			connectionId,
			kubeconfigEnvVar,
		),
		queryFn: () => getArgoApplicationResources(client, request),
		enabled:
			active &&
			workspaceReadContext.sourceReady &&
			(transport === "kubernetes" || Boolean(connectionId)),
		staleTime: 30_000,
		retry: false,
		gcTime: redactSecrets ? undefined : 0,
	}));
	const application = $derived<ArgoApplicationRef>({
		...applicationRequest,
		...(inspector.data?.application ?? {}),
		context: clusterContext,
		workspaceId,
	});
	const managedResources = $derived(resources.data ?? inspector.data?.resources ?? []);
	const reconciliationResources = $derived(argoReconciliationResources(managedResources));
	const comparableResources = $derived(
		reconciliationResources.filter((item) => argoResourceIdentityKey(item) !== null),
	);
	const comparisonQueries = createQueries(() => ({
		queries: comparisonSlots.map((item, index) => ({
			queryKey: queryKeys.argoWorkspaceComparison(
				clusterContext,
				workspaceId,
				resourceSummary.name,
				resourceSummary.namespace,
				applicationRequest.uid,
				redactSecrets,
				transport,
				connectionId,
				item?.group,
				item?.kind ?? "Disabled",
				item?.namespace,
				item?.name ?? `slot-${index}`,
				kubeconfigEnvVar,
			),
			queryFn: () => {
				if (!item) throw new Error("Comparison slot is disabled");
				return getArgoResourceComparison(client, { ...request, resource: item });
			},
			enabled:
				Boolean(item) &&
				active &&
				workspaceReadContext.sourceReady &&
				(transport === "kubernetes" || Boolean(connectionId)),
			staleTime: 30_000,
			retry: false,
			gcTime: redactSecrets ? undefined : 0,
		})),
	}));

	const scopeKey = $derived(
		[
			clusterContext,
			workspaceId,
			kubeconfigEnvVar ?? "",
			resourceSummary.namespace ?? "",
			resourceSummary.name,
			applicationRequest.uid ?? "",
		].join(":"),
	);
	const selectedHistory = $derived(
		(inspector.data?.history ?? []).find(
			(entry) => argoHistoryKey(application, entry) === selectedHistoryKey,
		) ?? inspector.data?.history[0] ?? null,
	);
	const diffResources = $derived(selectedResource ? [selectedResource] : reconciliationResources);
	const counts = $derived(argoResourceCounts(managedResources));
	const healthStatus = $derived(
		statusValue(inspector.data?.status, "health", "status") ??
			matchingSummary?.healthStatus ??
			resourceSummary.status ??
			"Unknown",
	);
	const syncStatus = $derived(
		statusValue(inspector.data?.status, "sync", "status") ??
			matchingSummary?.syncStatus ??
			"Unknown",
	);
	const resolvedRevision = $derived(
		statusValue(inspector.data?.status, "sync", "revision") ??
			matchingSummary?.sources?.[0]?.resolvedRevision ??
			selectedHistory?.revision ??
			matchingSummary?.sourceRevision ??
			"Revision unavailable",
	);
	const operationStateMessage = $derived(
		statusValue(inspector.data?.operationState, "phase") ??
			statusValue(inspector.data?.operationState, "message"),
	);
	const conditionRows = $derived(
		(inspector.data?.conditions ?? []).flatMap((condition) => {
			const type = statusValue(condition, "type");
			const message = statusValue(condition, "message");
			return type || message ? [{ type: type ?? "Application condition", message: message ?? "No condition detail reported." }] : [];
		}),
	);
	const applicationSpec = $derived(applicationSpecFromYaml(applicationDetails?.yaml));
	const primarySource = $derived(matchingSummary?.sources?.[0] ?? null);
	const repository = $derived(
		primarySource?.repoUrl ??
			matchingSummary?.sourceRepo ??
			stringAt(applicationSpec, "source", "repoURL") ??
			stringAt(applicationSpec, "sources", "0", "repoURL") ??
			"Repository unavailable",
	);
	const sourcePath = $derived(
		primarySource?.path ??
			primarySource?.chart ??
			stringAt(applicationSpec, "source", "path") ??
			stringAt(applicationSpec, "source", "chart") ??
			stringAt(applicationSpec, "sources", "0", "path") ??
			"Path unavailable",
	);
	const configuredRevision = $derived(
		primarySource?.targetRevision ??
			matchingSummary?.sourceRevision ??
			stringAt(applicationSpec, "source", "targetRevision") ??
			stringAt(applicationSpec, "sources", "0", "targetRevision") ??
			"Revision unavailable",
	);
	const destination = $derived(
		matchingSummary?.destinationNamespace ??
			stringAt(applicationSpec, "destination", "namespace") ??
			matchingSummary?.destinationServer ??
			stringAt(applicationSpec, "destination", "server") ??
			"Destination unavailable",
	);
	const applicationSyncDefaults = $derived<ArgoSyncSettings>({
		revision: "",
		prune: booleanAt(applicationSpec, "syncPolicy", "automated", "prune") ?? false,
		dryRun: false,
		force: false,
	});
	const policy = $derived(
		booleanAt(applicationSpec, "syncPolicy", "automated", "prune") === null &&
		booleanAt(applicationSpec, "syncPolicy", "automated", "selfHeal") === null
			? "Policy unavailable"
			: [
				"Automated",
				booleanAt(applicationSpec, "syncPolicy", "automated", "prune") ? "prune" : null,
				booleanAt(applicationSpec, "syncPolicy", "automated", "selfHeal") ? "self-heal" : null,
			].filter(Boolean).join(" · "),
	);
	const loading = $derived(inspector.isPending || resources.isPending);
	const dataError = $derived(
		inspector.isError ? inspector.error : resources.isError ? resources.error : null,
	);
	const busy = $derived(
		operationPhase === "authorizing" || operationPhase === "submitting" || operationPhase === "refreshing",
	);
	const canOperate = $derived(
		active &&
		workspaceReadContext.sourceReady &&
		(transport === "kubernetes" || Boolean(connectionId && selectedStatus?.connected)),
	);

	$effect(() => {
		const next = comparableResources;
		const slotCount = Math.max(comparisonSlots.length, next.length);
		const padded = Array.from({ length: slotCount }, (_, index) => next[index] ?? null);
		if (padded.some((item, index) => item !== comparisonSlots[index])) {
			// TanStack's Svelte adapter cannot safely shrink createQueries results; keep surplus observers disabled.
			comparisonSlots = padded;
		}
	});
	$effect(() => {
		const nextScopeKey = scopeKey;
		if (appliedScopeKey && appliedScopeKey !== nextScopeKey) {
			selectedResource = null;
			selectedHistoryKey = null;
			diffView = "changes";
			expandedRemovalKeys = [];
			operationPhase = "idle";
			operationMessage = null;
			operationError = null;
			acceptedRefreshPending = false;
			lastOperationRequest = null;
			pendingSync = null;
			confirmationOpen = false;
			confirmationName = "";
			appliedSyncDefaultsKey = "";
		}
		appliedScopeKey = nextScopeKey;
	});
	$effect(() => {
		const next = preserveArgoResourceSelection(selectedResource, reconciliationResources);
		if (next !== selectedResource) selectedResource = next;
	});
	$effect(() => {
		const next = preserveArgoHistorySelection(
			application,
			inspector.data?.history ?? [],
			selectedHistoryKey,
		);
		if (next !== selectedHistoryKey) selectedHistoryKey = next;
	});
	$effect(() => {
		const next = { ...applicationSyncDefaults };
		const key = JSON.stringify(next);
		if (appliedSyncDefaultsKey !== key) {
			advancedSync = appliedSyncDefaultsKey
				? applyArgoSyncDefaults(advancedSync, appliedSyncDefaults, next)
				: next;
			appliedSyncDefaults = next;
			appliedSyncDefaultsKey = key;
		}
	});

	function setTransport(value: string) {
		transport = value === "connected" ? "connected" : "kubernetes";
		operationError = null;
		operationMessage = null;
		operationPhase = "idle";
	}

	function setConnection(value: string) {
		connectionId = value;
		transport = "connected";
		operationError = null;
		operationMessage = null;
		operationPhase = "idle";
	}

	function operation(action: ArgoOperationAction): ArgoOperationRequest {
		return {
			connectionId: transport === "connected" ? connectionId : null,
			transport,
			application,
			action,
			revision: null,
			resources: [],
			prune: null,
			dryRun: null,
			force: null,
			historyId: null,
			resourceAction: null,
			resourceActionParameters: null,
			resourceVersion: application.resourceVersion ?? null,
			clusterContext,
			kubeconfigEnvVar,
		};
	}

	async function refreshApplicationState() {
		await Promise.all([
			queryClient.invalidateQueries({
				queryKey: queryKeys.argoWorkspaceApplication(
					clusterContext,
					workspaceId,
					resourceSummary.name,
					resourceSummary.namespace,
					applicationRequest.uid,
					redactSecrets,
					kubeconfigEnvVar,
				),
			}),
			queryClient.invalidateQueries({
				queryKey: queryKeys.argoApps(clusterContext, kubeconfigEnvVar),
			}),
		]);
	}

	async function executeOperation(requested: ArgoOperationRequest) {
		lastOperationRequest = requested;
		acceptedRefreshPending = false;
		operationError = null;
		operationMessage = "Checking authorization and operation scope…";
		operationPhase = "authorizing";
		try {
			await runArgoOperationLifecycle({
				request: requested,
				preflight: (request) => preflightArgoOperation(client, request),
				run: async (request) => {
					operationPhase = "submitting";
					operationMessage = `Submitting ${operationLabel(request.action)}…`;
					return runArgoOperation(client, request);
				},
				refresh: async () => {
					operationPhase = "refreshing";
					operationMessage = `${operationLabel(requested.action)} accepted; refreshing Application state…`;
					await refreshApplicationState();
				},
			});
			operationPhase = "accepted";
			operationMessage = `${operationLabel(requested.action)} accepted; latest Application state loaded. Completion follows Argo CD operation state.`;
		} catch (error) {
			operationPhase = "error";
			acceptedRefreshPending = error instanceof ArgoOperationRefreshError;
			operationError = error instanceof Error ? error.message : String(error);
			operationMessage = null;
		}
	}

	async function retryAcceptedRefresh() {
		if (!acceptedRefreshPending || busy) return;
		const label = lastOperationRequest
			? operationLabel(lastOperationRequest.action)
			: "Operation";
		operationError = null;
		operationPhase = "refreshing";
		operationMessage = `${label} accepted; retrying Application state refresh…`;
		try {
			await refreshApplicationState();
			acceptedRefreshPending = false;
			operationPhase = "accepted";
			operationMessage = `${label} accepted; latest Application state loaded. Completion follows Argo CD operation state.`;
		} catch (error) {
			operationPhase = "error";
			operationError = new ArgoOperationRefreshError(error).message;
			operationMessage = null;
		}
	}

	function refresh(hard = false) {
		refreshMenuOpen = false;
		void executeOperation(operation(hard ? "hardRefresh" : "refresh"));
	}

	function syncWithDefaults() {
		void executeOperation(
			withArgoSyncSettings(operation("sync"), applicationSyncDefaults),
		);
	}

	function requestAdvancedSync() {
		syncMenuOpen = false;
		const requested = { ...advancedSync };
		if (argoSyncNeedsConfirmation(requested, applicationSyncDefaults)) {
			pendingSync = requested;
			confirmationName = "";
			confirmationOpen = true;
			return;
		}
		void executeOperation(withArgoSyncSettings(operation("sync"), requested));
	}

	function confirmAdvancedSync() {
		if (!pendingSync || confirmationName !== resourceSummary.name) return;
		const requested = withArgoSyncSettings(operation("sync"), pendingSync);
		confirmationOpen = false;
		pendingSync = null;
		confirmationName = "";
		void executeOperation(requested);
	}

	function retryOperation() {
		if (lastOperationRequest && !busy) void executeOperation(lastOperationRequest);
	}

	function selectResource(item: ArgoManagedResource) {
		selectedResource = item;
	}

	function showAllChanges() {
		selectedResource = null;
	}

	function resourceSelected(item: ArgoManagedResource): boolean {
		if (!selectedResource) return false;
		const selectedKey = argoResourceIdentityKey(selectedResource);
		return selectedKey !== null && selectedKey === argoResourceIdentityKey(item);
	}

	function comparisonQuery(item: ArgoManagedResource) {
		const key = argoResourceIdentityKey(item);
		if (!key) return null;
		const index = comparisonSlots.findIndex(
			(slot) => slot !== null && argoResourceIdentityKey(slot) === key,
		);
		return index < 0 ? null : comparisonQueries[index];
	}

	function comparisonFor(item: ArgoManagedResource): ArgoResourceComparison | null {
		return (comparisonQuery(item)?.data as ArgoResourceComparison | undefined) ?? null;
	}

	function removalExpanded(item: ArgoManagedResource): boolean {
		const key = argoResourceIdentityKey(item);
		return key !== null && expandedRemovalKeys.includes(key);
	}

	function toggleRemoval(item: ArgoManagedResource) {
		const key = argoResourceIdentityKey(item);
		if (!key) return;
		expandedRemovalKeys = expandedRemovalKeys.includes(key)
			? expandedRemovalKeys.filter((itemKey) => itemKey !== key)
			: [...expandedRemovalKeys, key];
	}

	function resourceLabel(item: ArgoManagedResource): string {
		return `${item.kind ?? "Resource"} ${item.namespace ? `${item.namespace}/` : ""}${item.name ?? "unknown"}`;
	}

	function resourceModified(item: ArgoManagedResource): boolean {
		const comparison = argoComparisonDocument(item, comparisonFor(item));
		if (typeof comparison.modified === "boolean") return comparison.modified;
		return documentText(comparison.desired) !== documentText(comparison.normalizedLive);
	}

	function resourceAdded(item: ArgoManagedResource): boolean {
		const comparison = argoComparisonDocument(item, comparisonFor(item));
		return documentText(comparison.live) === "" && documentText(comparison.desired) !== "";
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
		if (cursor < source.length) segments.push({ text: source.slice(cursor), className: "" });
		return segments;
	}

	function historyRevision(entry: ArgoApplicationHistory): string {
		return entry.revision ?? (entry.revisions.join(", ") || "Unknown revision");
	}

	function documentText(value: unknown): string {
		if (typeof value === "string") return value.trimEnd();
		return value === undefined || value === null ? "" : stringify(value).trimEnd();
	}

	function statusValue(value: unknown, ...path: string[]): string | null {
		let current = value;
		for (const key of path) {
			if (!current || typeof current !== "object") return null;
			current = (current as Record<string, unknown>)[key];
		}
		return typeof current === "string" && current.trim() ? current : null;
	}

	function applicationSpecFromYaml(yaml: string | undefined): Record<string, unknown> {
		if (!yaml) return {};
		try {
			const value = parse(yaml);
			return value && typeof value === "object" && !Array.isArray(value)
				? ((value as Record<string, unknown>).spec as Record<string, unknown>) ?? {}
				: {};
		} catch {
			return {};
		}
	}

	function valueAt(value: unknown, ...path: string[]): unknown {
		let current = value;
		for (const key of path) {
			if (Array.isArray(current)) {
				const index = Number(key);
				if (!Number.isInteger(index)) return undefined;
				current = current[index];
				continue;
			}
			if (!current || typeof current !== "object") return undefined;
			current = (current as Record<string, unknown>)[key];
		}
		return current;
	}

	function stringAt(value: unknown, ...path: string[]): string | null {
		const current = valueAt(value, ...path);
		return typeof current === "string" && current.trim() ? current : null;
	}

	function booleanAt(value: unknown, ...path: string[]): boolean | null {
		const current = valueAt(value, ...path);
		return typeof current === "boolean" ? current : null;
	}

	function operationLabel(action: ArgoOperationAction): string {
		if (action === "hardRefresh") return "Hard refresh";
		if (action === "sync") return "Sync";
		return "Refresh";
	}
</script>

<div class="@container flex min-h-0 min-w-0 flex-col gap-3 [&_button]:motion-reduce:transition-none" data-active={active}>
	<section class="overflow-hidden rounded-lg border bg-surface-1 shadow-sm">
		<div class="resource-tone-argo-surface p-3">
			<div class="flex flex-wrap items-start justify-between gap-3">
				<div class="min-w-0">
					<div class="flex min-w-0 flex-wrap items-center gap-1.5">
						<h2 class="max-w-full truncate font-heading text-sm font-semibold">{resourceSummary.name}</h2>
						<Badge variant="ghost"><CircleDot class="size-2.5 text-primary" /> {inspector.data?.connected ? "Argo CD API" : "Application CRD"}</Badge>
					</div>
					<div class="mt-2 flex flex-wrap items-center gap-2">
						<div class="font-heading text-2xl font-semibold tracking-tight">{healthStatus}</div>
						<Badge variant="outline">{syncStatus}</Badge>
						{#if operationStateMessage}<Badge variant="secondary">{operationStateMessage}</Badge>{/if}
					</div>
					<div class="mt-1 max-w-xl text-xs leading-relaxed text-muted-foreground">
						{loading ? "Loading Application and managed resources…" : `Revision ${resolvedRevision}`}
					</div>
				</div>
				<div class="flex max-w-full shrink-0 flex-wrap gap-1.5">
					<div class="inline-flex shrink-0">
						<Button type="button" size="sm" variant="outline" class="rounded-r-none" disabled={!canOperate || busy} onclick={() => refresh(false)}>
							{#if operationPhase === "refreshing" && lastOperationRequest?.action !== "sync"}<Spinner />{:else}<RefreshCw data-icon="inline-start" />{/if}
							Refresh
						</Button>
						<Popover bind:open={refreshMenuOpen}>
							<PopoverTrigger type="button" class={buttonClass({ variant: "outline", size: "icon-sm", className: "rounded-l-none border-l-0" })} aria-label="Refresh options" disabled={!canOperate || busy}><ChevronDown /></PopoverTrigger>
							<PopoverContent align="end" class="w-64 space-y-2">
								<div class="text-sm font-semibold">Refresh Application</div>
								<div class="text-xs leading-relaxed text-muted-foreground">Normal refresh re-reads Application state. Hard refresh also invalidates Argo CD manifest and repository caches.</div>
								<Button type="button" variant="outline" size="sm" class="w-full" disabled={!canOperate || busy} onclick={() => refresh(true)}><RotateCcw data-icon="inline-start" /> Hard refresh</Button>
							</PopoverContent>
						</Popover>
					</div>
					<div class="inline-flex shrink-0">
						<Button type="button" size="sm" class="rounded-r-none" disabled={!canOperate || busy} onclick={syncWithDefaults}>
							{#if busy && lastOperationRequest?.action === "sync"}<Spinner />{:else}<RotateCcw data-icon="inline-start" />{/if}
							Sync
						</Button>
						<Popover bind:open={syncMenuOpen}>
							<PopoverTrigger type="button" class={buttonClass({ size: "icon-sm", className: "rounded-l-none border-l border-primary-foreground/20" })} aria-label="Sync options" disabled={!canOperate || busy}><ChevronDown /></PopoverTrigger>
							<PopoverContent align="end" class="w-80 max-w-[calc(100vw-2rem)] space-y-3">
								<div><div class="text-sm font-semibold">Sync with options</div><div class="mt-0.5 text-xs leading-relaxed text-muted-foreground">Main Sync uses Application defaults. Choices beyond those defaults require confirmation.</div></div>
								<Field><FieldLabel>Revision override</FieldLabel><Input value={advancedSync.revision} placeholder={configuredRevision} oninput={(event: Event) => advancedSync = { ...advancedSync, revision: (event.currentTarget as HTMLInputElement).value }} /></Field>
								<div class="grid grid-cols-2 gap-2">
									<label class="flex items-center gap-2 rounded-md border p-2 text-xs"><Checkbox checked={advancedSync.prune} onCheckedChange={(checked) => advancedSync = { ...advancedSync, prune: checked === true }} />Prune</label>
									<label class="flex items-center gap-2 rounded-md border p-2 text-xs"><Checkbox checked={advancedSync.dryRun} onCheckedChange={(checked) => advancedSync = { ...advancedSync, dryRun: checked === true }} />Dry run</label>
									<label class="col-span-2 flex items-center gap-2 rounded-md border p-2 text-xs"><Checkbox checked={advancedSync.force} onCheckedChange={(checked) => advancedSync = { ...advancedSync, force: checked === true }} />Force / replace</label>
								</div>
								<Button type="button" class="w-full" disabled={!canOperate || busy} onclick={requestAdvancedSync}>Sync with these options</Button>
							</PopoverContent>
						</Popover>
					</div>
				</div>
			</div>
			<div class="mt-3 grid grid-cols-3 gap-px overflow-hidden rounded-md border bg-border">
				{@render SignalCard(counts.needsSync, "Out of sync", "Needs reconciliation")}
				{@render SignalCard(counts.degraded + counts.progressing, "Attention", "Health is not settled")}
				{@render SignalCard(counts.prune, "Prune", "No longer in target")}
			</div>
		</div>
	</section>

	<div class="flex min-w-0 flex-wrap items-center gap-2 rounded-lg border bg-surface-1 p-3">
		<Select value={transport} onValueChange={setTransport}>
			<SelectTrigger class="w-56 max-w-full" ariaLabel="Argo CD access path"><SelectValue>{transport === "connected" ? "Connected Argo CD profile" : "Kubernetes fallback"}</SelectValue></SelectTrigger>
			<SelectContent><SelectGroup><SelectItem value="kubernetes" label="Kubernetes fallback">Kubernetes fallback</SelectItem><SelectItem value="connected" label="Connected Argo CD profile">Connected Argo CD profile</SelectItem></SelectGroup></SelectContent>
		</Select>
		{#if transport === "connected"}
			<Select value={connectionId} onValueChange={setConnection}>
				<SelectTrigger class="w-56 max-w-full" ariaLabel="Connected Argo CD profile"><SelectValue>{selectedProfile?.url ?? "Select connected profile"}</SelectValue></SelectTrigger>
				<SelectContent><SelectGroup>{#each matchingProfiles as profile}<SelectItem value={profile.id} label={profile.url}>{profile.url}</SelectItem>{/each}</SelectGroup></SelectContent>
			</Select>
		{/if}
		<Button size="sm" type="button" variant="outline" onclick={() => (connectionSettingsOpen = true)}>Connection settings</Button>
		<div class="min-w-0 flex-1 text-xs text-muted-foreground">
			{transport === "kubernetes" ? "Argo RBAC is not evaluated. Exact manifests require a connected profile." : connectionId && !selectedStatus?.connected ? selectedStatus?.unavailableReason ?? "Reconnect this profile." : "Connected Argo CD authorization is evaluated during operation preflight."}
		</div>
	</div>

	{#if operationMessage}
		<div class="flex items-center gap-2 rounded-lg border bg-surface-1 p-3 text-xs" role="status" aria-live="polite" aria-atomic="true">
			{#if busy}<Spinner class="size-3.5" />{/if}<span>{operationMessage}</span>
		</div>
	{/if}
	{#if operationError}
		<Alert variant="destructive">
			<AlertTitle>{acceptedRefreshPending ? "Application refresh failed" : "Operation failed"}</AlertTitle>
			<AlertDescription class="flex flex-wrap items-center justify-between gap-2"><span>{operationError}</span>{#if acceptedRefreshPending}<Button type="button" size="sm" variant="outline" disabled={busy} onclick={() => void retryAcceptedRefresh()}>Retry state refresh</Button>{:else if lastOperationRequest}<Button type="button" size="sm" variant="outline" disabled={busy} onclick={retryOperation}>Retry operation</Button>{/if}</AlertDescription>
		</Alert>
	{/if}
	{#if dataError}
		<Alert variant="destructive">
			<AlertTitle>Argo Application data unavailable</AlertTitle>
			<AlertDescription class="flex flex-wrap items-center justify-between gap-2"><span>{dataError instanceof Error ? dataError.message : String(dataError)}</span><Button type="button" size="sm" variant="outline" onclick={() => void Promise.all([inspector.refetch(), resources.refetch()])}>Retry</Button></AlertDescription>
		</Alert>
	{/if}

	{#each conditionRows as condition}
		<div class="flex items-start gap-2 rounded-lg border border-amber-500/25 bg-amber-500/5 p-3 text-xs">
			<TriangleAlert class="mt-0.5 size-3.5 shrink-0 text-amber-400" />
			<div class="min-w-0"><div class="font-medium">{condition.type}</div><div class="mt-0.5 break-words leading-relaxed text-muted-foreground">{condition.message}</div></div>
		</div>
	{/each}

	<section class="rounded-lg border bg-surface-1 p-3">
		<div class="flex items-center justify-between gap-2"><div><div class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Change navigator</div><div class="mt-0.5 text-xs text-muted-foreground">Focus one reconciliation item or keep complete stream.</div></div><Badge variant="outline">{reconciliationResources.length}</Badge></div>
		<div class="mt-3 flex gap-2 overflow-x-auto pb-1">
			<Button type="button" size="sm" variant={selectedResource ? "outline" : "secondary"} class="shrink-0" aria-pressed={!selectedResource} onclick={showAllChanges}>All changes</Button>
			{#each reconciliationResources as item (argoResourceIdentityKey(item) ?? resourceLabel(item))}
				<Button type="button" size="sm" variant={resourceSelected(item) ? "secondary" : "outline"} class="h-auto min-w-36 max-w-56 shrink-0 justify-start p-2 text-left" aria-pressed={resourceSelected(item)} onclick={() => selectResource(item)}>
					<span class="min-w-0"><span class="block truncate text-xs font-medium">{item.kind ?? "Resource"} · {item.name ?? "unknown"}</span><span class="mt-0.5 flex flex-wrap gap-1"><Badge variant="outline">{item.status ?? "Unknown"}</Badge>{#if item.requiresPruning}<Badge variant="destructive">Prune</Badge>{/if}</span></span>
				</Button>
			{/each}
		</div>
	</section>

	<section class="grid gap-3 @min-[36rem]:grid-cols-[minmax(0,1.35fr)_minmax(12rem,0.65fr)]">
		<div class="min-w-0 rounded-lg border bg-surface-1 p-3">
			<div class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Delivery</div>
			<div class="mt-3 grid min-w-0 grid-cols-[4.5rem_minmax(0,1fr)] gap-x-2 gap-y-2 text-xs">
				<span class="text-muted-foreground">Repository</span><span class="min-w-0 break-all">{repository}</span>
				<span class="text-muted-foreground">Path</span><span class="min-w-0 break-all">{sourcePath}</span>
				<span class="text-muted-foreground">Revision</span><span class="min-w-0 break-all">{configuredRevision}</span>
				<span class="text-muted-foreground">Destination</span><span class="min-w-0 break-all">{destination}</span>
				<span class="text-muted-foreground">Policy</span><span>{policy}</span>
			</div>
		</div>
		<div class="min-w-0 rounded-lg border bg-surface-1 p-3">
			<div class="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground"><GitCommitHorizontal class="size-3.5" /> Recent deployment</div>
			{#if inspector.data?.history.length}
				{#if inspector.data.history.length > 1}
					<Select value={selectedHistoryKey ?? ""} onValueChange={(value: string) => (selectedHistoryKey = value)}>
						<SelectTrigger class="mt-3 w-full" ariaLabel="Recent deployment"><SelectValue>{selectedHistory ? historyRevision(selectedHistory) : "Select deployment"}</SelectValue></SelectTrigger>
						<SelectContent><SelectGroup>{#each inspector.data.history as entry}<SelectItem value={argoHistoryKey(application, entry)} label={historyRevision(entry)}>{historyRevision(entry)}</SelectItem>{/each}</SelectGroup></SelectContent>
					</Select>
				{/if}
				{#if selectedHistory}<div class="mt-3 break-all text-sm font-semibold">{historyRevision(selectedHistory)}</div><div class="mt-1 text-xs leading-relaxed text-muted-foreground">{selectedHistory.deployedAt ?? "No deployment timestamp"} by {selectedHistory.initiatedBy ?? "unknown"}</div>{/if}
			{:else}
				<div class="mt-3 text-xs text-muted-foreground">No deployment history reported.</div>
			{/if}
		</div>
	</section>

	<section class="min-w-0 rounded-lg border bg-surface-1 p-3">
		<div class="flex flex-wrap items-start justify-between gap-2">
			<div><div class="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground"><GitCompareArrows class="size-3.5" /> {selectedResource ? "Focused reconciliation" : "Continuous change stream"}</div><div class="mt-1 break-all text-xs text-muted-foreground">{selectedResource ? resourceLabel(selectedResource) : `${diffResources.length} reconciliation items shown.`}</div></div>
			<div class="flex max-w-full flex-wrap items-center gap-2">{#if selectedResource}<Button type="button" size="xs" variant="outline" onclick={showAllChanges}>All changes</Button>{/if}<SegmentedControl value={diffView} options={diffViewOptions} onChange={(value) => (diffView = value)} ariaLabel="Reconciliation document view" size="sm" /></div>
		</div>
		<div class="mt-3 space-y-4">
			{#if loading}
				<div class="flex items-center gap-2 rounded-lg border p-3 text-xs text-muted-foreground" role="status"><Spinner /> Loading reconciliation stream</div>
			{:else if dataError}
				<Empty><EmptyHeader><EmptyTitle>Change stream unavailable</EmptyTitle><EmptyDescription>Retry Application data before evaluating reconciliation state.</EmptyDescription></EmptyHeader></Empty>
			{:else}
				{#each diffResources as item (argoResourceIdentityKey(item) ?? resourceLabel(item))}
				{@const query = comparisonQuery(item)}
				{@const comparison = comparisonFor(item)}
				{@const documents = argoComparisonDocument(item, comparison)}
				{@const removal = item.requiresPruning === true}
				<article class="min-w-0 overflow-hidden rounded-lg border bg-background">
					<div class="flex flex-wrap items-center gap-2 border-b bg-surface-1 px-3 py-2">
						<div class="min-w-0 flex-1 truncate text-xs font-semibold" title={resourceLabel(item)}>{resourceLabel(item)}</div>
						{#if removal}<Badge variant="destructive">Remove on sync</Badge>{:else if resourceAdded(item)}<Badge variant="outline" class="border-emerald-500/35 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300">Added</Badge>{:else if resourceModified(item)}<Badge variant="outline" class="border-amber-500/40 bg-amber-500/10 text-amber-700 dark:text-amber-300">Modified</Badge>{:else}<Badge variant="secondary">In sync</Badge>{/if}
						{#if removal}<Button type="button" size="xs" variant="ghost" aria-expanded={removalExpanded(item)} onclick={() => toggleRemoval(item)}>{removalExpanded(item) ? "Hide diff" : "Show diff"}</Button>{/if}
						{#if !selectedResource}<Button type="button" size="xs" variant="ghost" onclick={() => selectResource(item)}>Focus</Button>{/if}
					</div>
					{#if removal}
						<div class="bg-destructive/10 p-3 font-sans text-xs leading-relaxed"><div class="font-medium text-destructive">Live-only resource</div><div class="mt-1 text-muted-foreground">This resource is no longer present in desired revision. Sync with prune removes live object. Live YAML stays collapsed until Show diff is selected.</div></div>
					{/if}
					{#if query?.isError}
						<div class="flex flex-wrap items-center justify-between gap-2 border-t p-3 text-xs text-destructive"><span>{query.error instanceof Error ? query.error.message : String(query.error)}</span><Button type="button" size="xs" variant="outline" onclick={() => void query?.refetch()}>Retry comparison</Button></div>
					{:else if query?.isPending}
						<div class="flex items-center gap-2 border-t p-3 text-xs text-muted-foreground"><Spinner /> Loading comparison</div>
					{:else if removal && diffView !== "target" && !removalExpanded(item)}
						<span class="sr-only">Live YAML collapsed.</span>
					{:else if diffView === "target" && removal}
						<div class="border-t p-3 font-sans text-xs leading-relaxed"><div class="font-medium">No desired manifest</div><div class="mt-1 text-muted-foreground">Selected revision no longer contains this resource.</div></div>
					{:else if diffView === "changes" && documentText(documents.desired) && documentText(documents.normalizedLive)}
						<div class="overflow-x-auto border-t font-mono text-[0.6875rem] leading-5">
							{#each buildArgoResourceDiff(documentText(documents.desired), documentText(documents.normalizedLive)) as line}<div class={`whitespace-pre px-3 ${diffLineClassName(line.type)}`}>{#each highlightedDiffSegments(line) as segment}<span class={segment.className}>{segment.text}</span>{/each}</div>{/each}
						</div>
					{:else if diffView === "changes" && removal && documentText(documents.live)}
						<div class="border-t [&_.yaml-code-editor]:rounded-none [&_.yaml-code-editor]:border-0 [&_.yaml-code-editor]:shadow-none"><YamlCodeEditor value={documentText(documents.live)} editable={false} minHeight="180px" showErrorLens={false} /></div>
					{:else if diffView === "target" && documentText(documents.desired)}
						<div class="border-t [&_.yaml-code-editor]:rounded-none [&_.yaml-code-editor]:border-0 [&_.yaml-code-editor]:shadow-none"><YamlCodeEditor value={documentText(documents.desired)} editable={false} minHeight="180px" showErrorLens={false} /></div>
					{:else if diffView === "live" && documentText(documents.live)}
						<div class="border-t [&_.yaml-code-editor]:rounded-none [&_.yaml-code-editor]:border-0 [&_.yaml-code-editor]:shadow-none"><YamlCodeEditor value={documentText(documents.live)} editable={false} minHeight="180px" showErrorLens={false} /></div>
					{:else}
						<div class="border-t p-3 text-xs leading-relaxed text-muted-foreground">{comparison?.exact === false || transport === "kubernetes" ? "Exact target and live manifests are unavailable through Kubernetes fallback. Select a connected Argo CD profile for manifest comparison." : `No ${diffView === "target" ? "desired" : diffView === "live" ? "live" : "comparison"} manifest reported.`}</div>
					{/if}
				</article>
				{:else}
					<Empty><EmptyHeader><EmptyTitle>No pending changes</EmptyTitle><EmptyDescription>Inspector reports no out-of-sync, unhealthy, progressing, or removal resources.</EmptyDescription></EmptyHeader></Empty>
				{/each}
			{/if}
		</div>
	</section>
</div>

<Dialog open={connectionSettingsOpen} onOpenChange={(value: boolean) => (connectionSettingsOpen = value)}>
	<DialogContent class="max-w-2xl"><DialogHeader><DialogTitle>Argo CD connection settings</DialogTitle><DialogDescription>Manage connection profiles for this workspace.</DialogDescription></DialogHeader><ArgoConnectionSettings {clusterContext} {workspaceId} {kubeconfigEnvVar} /></DialogContent>
</Dialog>

<Dialog open={confirmationOpen} onOpenChange={(open: boolean) => { confirmationOpen = open; if (!open) { pendingSync = null; confirmationName = ""; } }}>
	<DialogContent>
		<DialogHeader><DialogTitle>Confirm advanced sync</DialogTitle><DialogDescription>These choices go beyond Application defaults. Type Application name to continue.</DialogDescription></DialogHeader>
		{#if pendingSync}
			<div class="grid grid-cols-[5rem_minmax(0,1fr)] gap-x-3 gap-y-1 rounded-md border bg-muted/20 p-3 text-xs">
				<span class="text-muted-foreground">Revision</span><span class="break-all">{pendingSync.revision || configuredRevision}</span>
				<span class="text-muted-foreground">Prune</span><span>{pendingSync.prune ? "Enabled" : "Disabled"}</span>
				<span class="text-muted-foreground">Dry run</span><span>{pendingSync.dryRun ? "Enabled" : "Disabled"}</span>
				<span class="text-muted-foreground">Force</span><span>{pendingSync.force ? "Enabled" : "Disabled"}</span>
			</div>
		{/if}
		<Field><FieldLabel>Type {resourceSummary.name} to confirm</FieldLabel><Input bind:value={confirmationName} /></Field>
		<div class="flex justify-end gap-2"><Button type="button" variant="outline" onclick={() => (confirmationOpen = false)}>Cancel</Button><Button type="button" disabled={confirmationName !== resourceSummary.name || !pendingSync} onclick={confirmAdvancedSync}>Confirm sync</Button></div>
	</DialogContent>
</Dialog>

{#snippet SignalCard(value: number, label: string, description: string)}
	<div class="bg-surface-1 p-2.5"><div class="flex items-baseline gap-1.5"><div class="text-base font-semibold tabular-nums">{value}</div><div class="text-xs font-medium">{label}</div></div><div class="mt-0.5 text-[0.65rem] text-muted-foreground">{description}</div></div>
{/snippet}
