<script lang="ts">
	import {
		AlertTriangle,
		Check,
		ChevronRight,
		Clipboard,
		KeyRound,
		Link2,
		RefreshCw,
		ShieldCheck,
	} from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import SurfaceFrame from "@/components/SurfaceFrame.svelte";
	import {
		Badge,
		Button,
		Card,
		CardContent,
	} from "@/components/ui/svelte";
	import type {
		RbacBindingSummary,
		RbacInspectionSummary,
		RbacNamespaceAccessSummary,
		RbacRoleSummary,
		ServiceAccountSummary,
	} from "@/lib/types";
	import { cnfast } from "@/lib/utils";
	import {
		buildRbacBindingIndex,
		cockpitItems,
		filterCockpitItems,
		ruleText,
		selectedCockpitItem,
		type RbacCockpitItem,
		type RbacCockpitState,
		type RbacRiskBucket,
	} from "./cockpitModel";
	import type { RbacVerifierHandoff } from "./handoff";
	import { grantSummary, observedPermissions, type InspectorIdentity } from "./observedPermissions";
	import RbacQueueFilters from "./RbacQueueFilters.svelte";
	import RbacReviewQueue from "./RbacReviewQueue.svelte";
	import RbacVerifier from "./RbacVerifier.svelte";
	import { identityLabel, rbacVerifier } from "./rbacVerifier.svelte";
	import RbacReviewBadge from "./RbacReviewBadge.svelte";
	import RbacReviewPanel from "./RbacReviewPanel.svelte";
	import type { RbacReviewRecord } from "./reviewModel";
	import { riskBadgeClass, riskRailClass, riskSummaryLabel } from "./risk";
	import { bindingSources } from "./queueModel";
	import type { RbacView } from "./surfaceModel";


	let {
		query,
		view,
		warningSummary,
		initialState,
		onStateChange,
		onViewChange,
		verifierHandoff,
		onVerifierHandoffConsumed,
		onVerifierReturn,
		verifierReturnLabel,
		reviewRecords,
		onReviewRecordsChange,
	}: {
		query: {
			data?: RbacInspectionSummary;
			isPending: boolean;
			isError: boolean;
			error: unknown;
			refetch?: () => void;
		};
		view: RbacView;
		warningSummary: (warnings: string[]) => string;
		initialState?: RbacCockpitState;
		onStateChange?: (state: RbacCockpitState) => void;
		onViewChange?: (view: RbacView) => void;
		verifierHandoff?: RbacVerifierHandoff;
		onVerifierHandoffConsumed?: () => void;
		onVerifierReturn?: () => void;
		verifierReturnLabel?: string;
		reviewRecords: RbacReviewRecord[];
		onReviewRecordsChange: (records: RbacReviewRecord[]) => void;
	} = $props();

	let bucket = $state<RbacRiskBucket>("all");
	let selectedKey = $state<string | undefined>();
	let search = $state("");
	let visible = $state(50);
	let copied = $state(false);

	$effect(() => {
		if (!initialState) return;
		bucket = initialState.riskBucket ?? "all";
		selectedKey = initialState.selectedObjectKey;
	});


	const data = $derived(query.data);
	const bindingIndex = $derived(data ? buildRbacBindingIndex(data) : null);
	const allItems = $derived(data && bindingIndex ? cockpitItems(data, view, bindingIndex) : []);
	const items = $derived(filterCockpitItems(allItems, bucket, search));
	const selected = $derived(selectedCockpitItem(items, selectedKey));
	const coverage = $derived(data?.coverage ?? []);
	const completeCoverage = $derived(
		coverage.filter((item) => item.status === "complete").length,
	);
	const allObjects = $derived(
		data
			? [
					...data.serviceAccounts,
					...data.roles,
					...data.clusterRoles,
					...data.roleBindings,
					...data.clusterRoleBindings,
				]
			: [],
	);
	const highObjects = $derived(
		allObjects.filter((item) => item.risks.some((risk) => risk.level === "high"))
			.length,
	);
	const unknownObjects = $derived(
		allObjects.filter((item) => item.risks.some((risk) => risk.level === "unknown"))
			.length,
	);
	const selectedIdentity = $derived<InspectorIdentity | null>(
		selected?.category === "Service Accounts" && isServiceAccountSummary(selected.item)
			? {
					kind: "serviceAccount",
					name: selected.item.name,
					namespace: selected.item.namespace,
				}
			: null,
	);
	const verifier = rbacVerifier({
		data: () => data,
		selectedIdentity: () => selectedIdentity,
		handoff: () => verifierHandoff,
		onHandoffConsumed: () => onVerifierHandoffConsumed?.(),
	});
	const activeIdentity = $derived(verifier.activeIdentity);
	const observed = $derived(
		data && bindingIndex && activeIdentity.name
			? observedPermissions(data, activeIdentity, bindingIndex)
			: null,
	);
	const selectedSources = $derived(
		bindingIndex && selected ? bindingSources(bindingIndex, selected) : [],
	);
	function select(key: string) {
		selectedKey = key;
		verifier.useSelection(true);
		onStateChange?.({ riskBucket: bucket, selectedObjectKey: key });
	}

	function setView(next: RbacView) {
		if (next === view) return;
		search = "";
		visible = 50;
		selectedKey = undefined;
		verifier.useSelection();
		onStateChange?.({ riskBucket: bucket, selectedObjectKey: undefined });
		onViewChange?.(next);
	}

	function setBucket(next: RbacRiskBucket) {
		bucket = next;
		visible = 50;
		onStateChange?.({ riskBucket: next, selectedObjectKey: selectedKey });
	}

	function copyEvidence() {
		if (!selected) return;
		void navigator.clipboard?.writeText(JSON.stringify(selected.item, null, 2));
		copied = true;
		setTimeout(() => (copied = false), 1500);
	}

	function selectedReach(): string {
		if (!observed || observed.grants.length === 0) return observed?.unknown ? "Unknown" : "No grants";
		if (observed.grants.some((grant) => grant.scope === "cluster")) return "Cluster-wide";
		const namespaces = new Set(
			observed.grants.map((grant) => grant.namespace).filter(Boolean),
		);
		return `${namespaces.size} namespace${namespaces.size === 1 ? "" : "s"}`;
	}

	function sourceLabel(binding: RbacBindingSummary): string {
		return `${binding.kind}/${binding.namespace ? `${binding.namespace}/` : ""}${binding.name}`;
	}

	function selectedSubtitle(entry: RbacCockpitItem): string {
		if (entry.category === "Service Accounts") return "identity investigation";
		if (entry.category === "Bindings") return "subject and role investigation";
		if (entry.category === "Roles" || entry.category === "Cluster Roles") return "policy investigation";
		return "namespace access investigation";
	}

	function policyEvidenceLabel(entry: RbacCockpitItem): string {
		return entry.category === "Service Accounts" ? "Token posture" : "Policy shape";
	}

	function policyEvidenceValue(entry: RbacCockpitItem): string {
		if (entry.category === "Service Accounts" && isServiceAccountSummary(entry.item)) {
			const account = entry.item;
			return account.automountToken === true
				? "Automount on"
				: account.automountToken === false
					? "Automount off"
					: "Not reported";
		}
		if ((entry.category === "Roles" || entry.category === "Cluster Roles") && isRbacRoleSummary(entry.item)) {
			return `${entry.item.rulesCount} rules`;
		}
		if (entry.category === "Bindings" && isRbacBindingSummary(entry.item)) {
			return `${entry.item.subjects.length} subjects`;
		}
		return isRbacNamespaceAccessSummary(entry.item) ? `${entry.item.roles} roles` : "No loaded evidence";
	}

	function isRbacRoleSummary(item: RbacCockpitItem["item"]): item is RbacRoleSummary {
		return "rules" in item;
	}

	function isRbacBindingSummary(item: RbacCockpitItem["item"]): item is RbacBindingSummary {
		return "roleRefKind" in item;
	}

	function isServiceAccountSummary(item: RbacCockpitItem["item"]): item is ServiceAccountSummary {
		return "imagePullSecretsCount" in item;
	}

	function isRbacNamespaceAccessSummary(item: RbacCockpitItem["item"]): item is RbacNamespaceAccessSummary {
		return "roleBindings" in item;
	}

	function coverageClass(status: "complete" | "partial" | "unavailable") {
		if (status === "complete") return "text-emerald-600 dark:text-emerald-300";
		if (status === "partial") return "text-amber-600 dark:text-amber-300";
		return "text-destructive";
	}

	function freshnessLabel(value?: string): string {
		if (!value) return "freshness unavailable";
		const ageSeconds = Math.max(0, Math.floor((Date.now() - new Date(value).getTime()) / 1000));
		if (ageSeconds < 60) return `refreshed ${ageSeconds}s ago`;
		if (ageSeconds < 3600) return `refreshed ${Math.floor(ageSeconds / 60)}m ago`;
		return `refreshed ${new Date(value).toLocaleString()}`;
	}

</script>

<SurfaceFrame
	icon={KeyRound}
	title="RBAC Cockpit"
	{query}
	wide
	errorLabel="RBAC inspection unavailable"
>
	{@const inspection = data}
	{#if inspection && bindingIndex}
		<div class="flex min-h-0 flex-col gap-3">
			<div class="flex flex-wrap items-center gap-2">
				<Badge variant="outline" class="border-emerald-500/35 bg-emerald-500/10 text-emerald-600 dark:text-emerald-300">
					{coverage.length > 0 && completeCoverage === coverage.length ? "Coverage complete" : "Coverage partial"}
				</Badge>
				<Badge variant="outline">Entire cluster</Badge>
			</div>

			<Card size="sm" elevation="flat">
				<CardContent class="grid gap-3 p-0 xl:grid-cols-[minmax(14rem,1.1fr)_minmax(30rem,2.4fr)_auto] xl:items-stretch">
					<div class="px-3 py-2.5">
						<p class="text-sm font-semibold">Access posture</p>
						<p class="mt-0.5 text-xs text-muted-foreground">
							{inspection.cluster} · {completeCoverage}/{coverage.length || 5} RBAC families · {freshnessLabel(inspection.refreshedAt)}
						</p>
					</div>
					<div class="grid grid-cols-2 border-y border-border sm:grid-cols-5 xl:border-x xl:border-y-0">
						{#each [
							["Identities", inspection.serviceAccounts.length],
							["Roles", inspection.roles.length + inspection.clusterRoles.length],
							["Bindings", inspection.roleBindings.length + inspection.clusterRoleBindings.length],
							["High", highObjects],
							["Unknown", unknownObjects],
						] as metric}
							<div class="border-r border-border px-3 py-2 last:border-r-0">
								<p class="text-[0.625rem] font-medium uppercase tracking-wide text-muted-foreground">{metric[0]}</p>
								<p class={cnfast("mt-0.5 text-base font-semibold tabular-nums", metric[0] === "High" && highObjects > 0 && "text-destructive", metric[0] === "Unknown" && unknownObjects > 0 && "text-amber-600 dark:text-amber-300")}>{metric[1]}</p>
							</div>
						{/each}
					</div>
					<div class="flex items-center justify-end gap-2 px-3 py-2.5">
						<Button size="sm" variant="outline" onclick={() => query.refetch?.()}>
							<RefreshCw data-icon /> Refresh
						</Button>
						<Button size="sm" onclick={() => verifier.beginIdentity()}>
							<ShieldCheck data-icon /> Investigate identity
						</Button>
					</div>
				</CardContent>
			</Card>

			<RbacQueueFilters {inspection} {view} {allItems} {bucket} bind:search {setView} {setBucket} />

			{#if verifier.handoffPromoted}
				<div class="flex flex-wrap items-center gap-3 rounded-lg border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs">
					<AlertTriangle class="size-4 text-amber-600 dark:text-amber-300" />
					<div class="mr-auto">
						<p class="font-semibold">Required permission ready to verify</p>
						<p class="text-muted-foreground">The denied action supplied exact identity and request context. Nothing runs until Verify access.</p>
					</div>
					{#if onVerifierReturn}
						<Button size="sm" variant="outline" onclick={onVerifierReturn}>{verifierReturnLabel ?? "Return"}</Button>
					{/if}
				</div>
			{/if}

			<div class="grid min-h-[calc(100vh-18rem)] gap-3 xl:grid-cols-[minmax(21rem,2fr)_minmax(35rem,3fr)]">
				<RbacReviewQueue {inspection} {bindingIndex} {view} {items} {selected} bind:visible {select} {reviewRecords} />

				<Card size="sm" elevation="flat" class="min-h-0 gap-0 py-0">
					<div class="flex flex-wrap items-start gap-3 border-b border-border px-3 py-3">
						<div class="min-w-0 flex-1">
							<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">
								{selected ? `${selected.kind} · ${selectedSubtitle(selected)}` : "Expert inspector"}
							</p>
							<h3 class="mt-1 truncate text-base font-semibold">{selected ? `${selected.kind}/${selected.name}` : "Select an RBAC object"}</h3>
							<p class="mt-0.5 truncate text-xs text-muted-foreground">
								{selected ? `${selected.namespace ?? "Cluster scope"} · ${identityLabel(activeIdentity)}` : "Exact evidence, policy findings, and permission sources"}
							</p>
						</div>
						{#if selected}
							<Badge variant="outline" class={riskBadgeClass(selected.risks)}>{riskSummaryLabel(selected.risks)}</Badge>
							<RbacReviewBadge {inspection} {bindingIndex} entry={selected} records={reviewRecords} />
						{/if}
						<Button size="sm" variant="outline" onclick={copyEvidence} disabled={!selected}>
							{#if copied}<Check data-icon /> Copied{:else}<Clipboard data-icon /> Copy evidence{/if}
						</Button>
					</div>

					<CardContent class="min-h-0 flex-1 overflow-y-auto p-3">
						<!-- The verifier stays beside the placeholder, so an identity can be checked when no object matches. -->
						<div class="grid gap-3 lg:grid-cols-[minmax(0,1.45fr)_minmax(17rem,.75fr)]">
							{#if selected}
								<div class="min-w-0 space-y-3">
									<div class="grid gap-2 sm:grid-cols-3">
										<div class="rounded-md border bg-muted/20 p-2.5">
											<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Reach</p>
											<p class="mt-1 text-sm font-semibold">{selectedReach()}</p>
											<p class="mt-0.5 text-xs text-muted-foreground">Observed policy scope</p>
										</div>
										<div class="rounded-md border bg-muted/20 p-2.5">
											<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Sources</p>
											<p class="mt-1 text-sm font-semibold">{selectedSources.length} binding{selectedSources.length === 1 ? "" : "s"}</p>
											<p class="mt-0.5 text-xs text-muted-foreground">Role and binding provenance</p>
										</div>
										<div class="rounded-md border bg-muted/20 p-2.5">
											<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">
											{policyEvidenceLabel(selected)}
											</p>
											<p class="mt-1 text-sm font-semibold">
											{policyEvidenceValue(selected)}
											</p>
											<p class="mt-0.5 text-xs text-muted-foreground">Exact loaded evidence</p>
										</div>
									</div>

									<section class="rounded-md border">
										<div class="border-b border-border px-3 py-2">
											<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Why this needs review</p>
										</div>
										<div class="space-y-2 p-2">
											{#each selected.risks as risk}
												<div class={cnfast("rounded-sm border border-l-[3px] p-2", riskRailClass([risk]))}>
													<div class="flex items-center gap-2">
														<p class="text-xs font-semibold">{risk.label}</p>
														<Badge variant="outline" class={riskBadgeClass([risk])}>{risk.level}</Badge>
													</div>
													<p class="mt-1 text-xs text-muted-foreground">{risk.reason}</p>
												</div>
											{:else}
												<p class="p-2 text-xs text-muted-foreground">No policy heuristic flags for this object.</p>
											{/each}
										</div>
									</section>

									{#if "rules" in selected.item}
										<section class="rounded-md border">
											<div class="border-b border-border px-3 py-2">
												<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Exact rules</p>
											</div>
											<div class="space-y-1.5 p-2">
												{#each selected.item.rules as rule}
													<p class="break-words rounded-sm bg-muted/35 px-2 py-1.5 font-mono text-xs">{ruleText(rule)}</p>
												{:else}
													<p class="p-2 text-xs text-muted-foreground">No rules.</p>
												{/each}
											</div>
										</section>
									{/if}

									{#if "subjects" in selected.item}
										<section class="rounded-md border">
											<div class="border-b border-border px-3 py-2">
												<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Subjects · {selected.item.roleRefKind}/{selected.item.roleRefName}</p>
											</div>
											<div class="divide-y divide-border px-2">
												{#each selected.item.subjects as subject}
													<button class="flex w-full items-center gap-2 py-2 text-left text-xs hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring" onclick={() => verifier.inspectSubject(subject)}>
														<span class="min-w-0 flex-1 truncate">{subject.kind}/{subject.namespace ? `${subject.namespace}/` : ""}{subject.name}</span>
														<span class="text-muted-foreground">Investigate</span><ChevronRight class="size-3" />
													</button>
												{:else}
													<p class="p-2 text-xs text-muted-foreground">No subjects.</p>
												{/each}
											</div>
										</section>
									{/if}

									<section class="rounded-md border">
										<div class="border-b border-border px-3 py-2">
											<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Observed RBAC grants</p>
											<p class="mt-0.5 text-xs text-muted-foreground">Policy-derived, not an authorization verdict.</p>
										</div>
										<div class="space-y-1.5 p-2">
											{#if observed?.unknown}<p class="rounded-sm border border-amber-500/30 bg-amber-500/10 p-2 text-xs text-amber-700 dark:text-amber-200">Unknown · {observed.reason}</p>{/if}
											{#each observed?.grants ?? [] as grant}
												<div class="rounded-sm border bg-muted/20 p-2">
													<p class="font-mono text-xs">{grantSummary(grant)}</p>
													<p class="mt-1 text-xs text-muted-foreground">{grant.scope}{grant.namespace ? `/${grant.namespace}` : ""} · {grant.roles.join(", ")}</p>
												</div>
											{:else}
												<p class="p-2 text-xs text-muted-foreground">{activeIdentity.name ? "No observed grants for this identity." : "Choose an identity to inspect grants."}</p>
											{/each}
										</div>
									</section>

									<section class="rounded-md border">
										<div class="flex items-center gap-2 border-b border-border px-3 py-2">
											<Link2 class="size-3.5 text-muted-foreground" />
											<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Source chain</p>
										</div>
										<div class="divide-y divide-border px-2">
											{#each selectedSources as source}
												<div class="flex items-center gap-2 py-2 text-xs">
													<span class="min-w-0 flex-1 truncate">{sourceLabel(source)}</span>
													<span class="truncate text-muted-foreground">{source.roleRefKind}/{source.roleRefName}</span>
												</div>
											{:else}
												<p class="p-2 text-xs text-muted-foreground">No loaded binding sources.</p>
											{/each}
										</div>
									</section>
								</div>
							{:else}
								<div class="flex min-h-64 min-w-0 items-center justify-center text-sm text-muted-foreground">Select an RBAC object to inspect evidence.</div>
							{/if}

							<aside class="min-w-0 space-y-3 lg:sticky lg:top-0 lg:self-start">
								{#if selected}
									<RbacReviewPanel
										{inspection}
										{bindingIndex}
										entry={selected}
										records={reviewRecords}
										onRecordsChange={onReviewRecordsChange}
									/>
								{/if}
								<RbacVerifier model={verifier} cluster={inspection.cluster} />

								<section class="rounded-md border p-3">
									<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Input coverage</p>
									<div class="mt-2 space-y-1.5">
										{#each coverage as item}
											<div class="flex items-center gap-2 text-xs">
												<span class="min-w-0 flex-1 truncate">{item.family}</span>
												<span class={cnfast("capitalize", coverageClass(item.status))}>{item.status}</span>
												<span class="w-6 text-right tabular-nums text-muted-foreground">{item.count}</span>
											</div>
										{/each}
									</div>
								</section>
							</aside>
						</div>
					</CardContent>
				</Card>
			</div>

			{#if inspection.warnings.length > 0}
				<FriendlyError
					mode="compact"
					error={warningSummary(inspection.warnings)}
					context={{
						operation: "resourcesLoad",
						fallbackTitle: "Partial RBAC data",
						partial: true,
					}}
				/>
			{/if}
		</div>
	{/if}
</SurfaceFrame>
