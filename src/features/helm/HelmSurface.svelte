<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import type { HealthFilter } from "@/features/resources";
	import { queryKeys } from "@/lib/queryKeys";
	import {
		createTauriClient,
		getHelmReleaseDetails,
		getHelmReleaseReconciliation,
		listHelmReleases,
	} from "@/lib/tauri";
	import type {
		ArgoApplicationSummary,
		HelmReconciliationResource,
		HelmReleaseDetails,
		HelmReleaseReconciliation,
		HelmReleaseSummary,
	} from "@/lib/types";
	import type { SavedWorkspace } from "@/lib/workspace-model";
	import {
		helmReconciliationStatusTone,
		helmReleaseKey,
		helmStatusTone,
		sortHelmReconciliationResources,
	} from "./helpers";
	import HelmView from "./HelmView.svelte";
	import {
		buildHelmReleaseState,
		resolveTargetHelmRelease,
		selectedHelmReleaseExists,
	} from "./surfaceState";

	let {
		workspace,
		sourceReady,
		kubeconfigSourceKey,
		targetHelmRelease = null,
		helmSearch = $bindable(""),
		selectedHelmRelease = $bindable(null),
		onTargetHelmReleaseResolved = () => {},
		onOpenResources,
	}: {
		workspace: SavedWorkspace;
		sourceReady: boolean;
		kubeconfigSourceKey?: string;
		targetHelmRelease?: { name: string; namespace?: string | null } | null;
		helmSearch?: string;
		selectedHelmRelease?: HelmReleaseSummary | null;
		onTargetHelmReleaseResolved?: () => void;
		onOpenResources: (
			namespace?: string | string[],
			initialSearch?: string,
			initialGitOpsFilter?: string,
			initialHealthFilter?: HealthFilter,
			gitOpsFocusApplication?: ArgoApplicationSummary | null,
		) => void;
	} = $props();

	const client = createTauriClient();
	const context = $derived(workspace.scope.clusterContext);
	const helmQuery = createQuery<HelmReleaseSummary[]>(() => ({
		queryKey: queryKeys.helmReleases(context, kubeconfigSourceKey),
		queryFn: () => listHelmReleases(client, context, kubeconfigSourceKey),
		enabled: sourceReady,
		staleTime: 15_000,
	}));
	const selectedHelmReleaseKey = $derived(
		selectedHelmRelease ? helmReleaseKey(selectedHelmRelease) : "",
	);
	const helmDetailsQuery = createQuery<HelmReleaseDetails>(() => ({
		queryKey: selectedHelmRelease
			? queryKeys.helmReleaseDetails(
					selectedHelmRelease.cluster,
					selectedHelmRelease.namespace,
					selectedHelmRelease.storageKind,
					selectedHelmRelease.storageName,
					kubeconfigSourceKey,
					"kubectl",
					"yaml",
				)
			: ["helm-release-details", "idle"],
		queryFn: () =>
			getHelmReleaseDetails(
				client,
				selectedHelmRelease as HelmReleaseSummary,
				kubeconfigSourceKey,
				"kubectl",
				"yaml",
			),
		enabled: sourceReady && Boolean(selectedHelmRelease),
		staleTime: 15_000,
	}));
	const helmReconciliationQuery = createQuery<HelmReleaseReconciliation>(() => ({
		queryKey: selectedHelmRelease
			? queryKeys.helmReleaseReconciliation(
					selectedHelmRelease.cluster,
					selectedHelmRelease.namespace,
					selectedHelmRelease.storageKind,
					selectedHelmRelease.storageName,
					kubeconfigSourceKey,
				)
			: ["helm-release-reconciliation", "idle"],
		queryFn: () =>
			getHelmReleaseReconciliation(
				client,
				selectedHelmRelease as HelmReleaseSummary,
				kubeconfigSourceKey,
			),
		enabled: sourceReady && Boolean(selectedHelmRelease),
		staleTime: 30_000,
	}));
	const releaseState = $derived(buildHelmReleaseState(helmQuery.data ?? [], helmSearch));
	const helmReconciliationRows = $derived(
		sortHelmReconciliationResources(helmReconciliationQuery.data?.resources ?? []),
	);

	$effect(() => {
		const target = resolveTargetHelmRelease(helmQuery.data, targetHelmRelease);
		if (target) {
			selectedHelmRelease = target;
			onTargetHelmReleaseResolved();
			return;
		}
		if (!selectedHelmReleaseExists(helmQuery.data, selectedHelmRelease)) {
			selectedHelmRelease = null;
		}
	});

	function helmStatusVariant(status: string | undefined) {
		return helmStatusTone(status) === "error" ? "destructive" : "outline";
	}

	function helmReconciliationSource(resource: HelmReconciliationResource): string {
		if (resource.inManifest && resource.liveResource) return "Manifest + live";
		if (resource.inManifest) return "Manifest";
		if (resource.liveResource) return "Live label";
		return "-";
	}

	function helmReconciliationClass(status: HelmReconciliationResource["status"]) {
		const tone = helmReconciliationStatusTone(status);
		if (tone === "success") return "border-emerald-500/40 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300";
		if (tone === "warning") return "border-amber-500/40 bg-amber-500/10 text-amber-700 dark:text-amber-300";
		if (tone === "error") return "border-destructive/40 bg-destructive/10 text-destructive";
		return "";
	}
</script>

<HelmView
	{helmQuery}
	groupedHelmReleases={releaseState.groups}
	filteredHelmReleases={releaseState.filtered}
	bind:helmSearch
	bind:selectedHelmRelease
	{selectedHelmReleaseKey}
	{helmDetailsQuery}
	{helmReconciliationQuery}
	{helmReconciliationRows}
	{onOpenResources}
	{helmStatusVariant}
	{helmReconciliationClass}
	{helmReconciliationSource}
/>
