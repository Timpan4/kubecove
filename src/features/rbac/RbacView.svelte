<script lang="ts">
	import {
		AlertTriangle,
		Check,
		ChevronRight,
		Clipboard,
		KeyRound,
		Link2,
		RefreshCw,
		Search,
		ShieldCheck,
	} from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import SurfaceFrame from "@/components/SurfaceFrame.svelte";
	import {
		Badge,
		Button,
		Card,
		CardContent,
		Input,
	} from "@/components/ui/svelte";
	import { createTauriClient, reviewRbacAccess } from "@/lib/tauri";
	import type {
		RbacAccessReviewRequest,
		RbacAccessReviewResult,
		RbacBindingSummary,
		RbacInspectionSummary,
		RbacNamespaceAccessSummary,
		RbacRiskIndicator,
		RbacRoleSummary,
		ServiceAccountSummary,
	} from "@/lib/types";
	import { cnfast } from "@/lib/utils";
	import {
		cockpitItems,
		filterCockpitItems,
		ruleText,
		selectedCockpitItem,
		type RbacCockpitItem,
		type RbacCockpitState,
		type RbacRiskBucket,
	} from "./cockpitModel";
	import type { RbacVerifierHandoff } from "./handoff";
	import { observedPermissions, identityDefaults, inspectorIdentity } from "./observedPermissions";
	import { riskSummaryLabel, subjectListLabel } from "./risk";
	import type { RbacView } from "./surfaceModel";

	type InspectorIdentity = {
		kind: "serviceAccount" | "user" | "group";
		name: string;
		namespace?: string;
		groups?: string[];
	};

	const categories: RbacView[] = [
		"Namespace Access",
		"Roles",
		"Cluster Roles",
		"Bindings",
		"Service Accounts",
	];
	const riskFilters: Array<[RbacRiskBucket, string]> = [
		["all", "All"],
		["high", "High"],
		["medium", "Medium"],
		["low", "Low"],
		["none", "No flags"],
		["unknown", "Unknown"],
	];

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
	} = $props();

	const client = createTauriClient();
	let bucket = $state<RbacRiskBucket>("all");
	let selectedKey = $state<string | undefined>();
	let search = $state("");
	let visible = $state(50);
	let copied = $state(false);
	let drillIdentity = $state<InspectorIdentity | null>(null);
	let manualIdentity = $state(false);
	let handoffPromoted = $state(false);
	let review = $state<RbacAccessReviewResult | null>(null);
	let reviewing = $state(false);
	let verb = $state("get");
	let resource = $state("pods");
	let apiGroup = $state("");
	let subresource = $state("");
	let resourceName = $state("");
	let namespace = $state("");
	let targetKind = $state<"resource" | "nonResource">("resource");
	let nonResourceUrl = $state("/healthz");
	let handoffKey = $state("");
	let identityKind = $state<"serviceAccount" | "user" | "group">("user");
	let identityName = $state("");
	let identityNamespace = $state("default");
	let identityGroups = $state("system:authenticated");
	let verifierError = $state("");
	let submitted = $state<{
		timestamp: string;
		fingerprint: string;
		identity: string;
		target: string;
	} | null>(null);
	let requestVersion = 0;

	$effect(() => {
		if (!initialState) return;
		bucket = initialState.riskBucket ?? "all";
		selectedKey = initialState.selectedObjectKey;
	});

	$effect(() => {
		const handoff = verifierHandoff;
		if (!handoff) return;
		const nextKey = JSON.stringify(handoff);
		if (handoffKey === nextKey) return;
		handoffKey = nextKey;
		handoffPromoted = true;
		drillIdentity = inspectorIdentity(handoff.identity);
		manualIdentity = drillIdentity === null;
		if (handoff.target.kind === "resource") {
			targetKind = "resource";
			verb = handoff.target.verb;
			resource = handoff.target.resource;
			apiGroup = handoff.target.apiGroup ?? "";
			namespace = handoff.target.namespace ?? "";
			subresource = handoff.target.subresource ?? "";
			resourceName = handoff.target.name ?? "";
		} else {
			targetKind = "nonResource";
			verb = handoff.target.verb;
			nonResourceUrl = handoff.target.nonResourceUrl;
		}
		review = null;
		onVerifierHandoffConsumed?.();
	});

	const data = $derived(query.data);
	const allItems = $derived(data ? cockpitItems(data, view) : []);
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
		selected?.category === "Service Accounts"
			? {
					kind: "serviceAccount",
					name: (selected.item as ServiceAccountSummary).name,
					namespace: (selected.item as ServiceAccountSummary).namespace,
				}
			: null,
	);
	const activeIdentity = $derived<InspectorIdentity>(
		drillIdentity ??
			(!manualIdentity && selectedIdentity
				? selectedIdentity
				: manualIdentityValue()),
	);
	const observed = $derived(
		data && activeIdentity.name
			? observedPermissions(data, activeIdentity)
			: null,
	);
	const selectedSources = $derived(
		data && selected ? bindingSources(data, selected) : [],
	);
	const fingerprint = $derived(
		JSON.stringify({
			cluster: data?.cluster,
			refreshedAt: data?.refreshedAt,
			activeIdentity,
			targetKind,
			verb,
			apiGroup,
			resource,
			namespace,
			subresource,
			resourceName,
			nonResourceUrl,
		}),
	);

	$effect(() => {
		fingerprint;
		review = null;
		verifierError = "";
		submitted = null;
		requestVersion += 1;
		reviewing = false;
	});

	function manualIdentityValue(): InspectorIdentity {
		if (identityKind === "serviceAccount") {
			return {
				kind: "serviceAccount",
				name: identityName.trim(),
				namespace: identityNamespace.trim(),
			};
		}
		if (identityKind === "group") {
			return { kind: "group", name: identityName.trim() };
		}
		return {
			kind: "user",
			name: identityName.trim(),
			groups: uniqueGroups(identityGroups),
		};
	}

	function select(key: string) {
		selectedKey = key;
		drillIdentity = null;
		manualIdentity = false;
		handoffPromoted = false;
		onStateChange?.({ riskBucket: bucket, selectedObjectKey: key });
	}

	function setView(next: RbacView) {
		if (next === view) return;
		search = "";
		visible = 50;
		selectedKey = undefined;
		drillIdentity = null;
		manualIdentity = false;
		onStateChange?.({ riskBucket: bucket, selectedObjectKey: undefined });
		onViewChange?.(next);
	}

	function setBucket(next: RbacRiskBucket) {
		bucket = next;
		visible = 50;
		onStateChange?.({ riskBucket: next, selectedObjectKey: selectedKey });
	}

	function beginIdentity(kind: "serviceAccount" | "user" | "group" = "user") {
		drillIdentity = null;
		manualIdentity = true;
		identityKind = kind;
		identityName = "";
		identityNamespace = "default";
		identityGroups = identityDefaults(kind).join(", ");
	}

	function inspectSubject(subject: {
		kind: string;
		name: string;
		namespace?: string;
	}) {
		drillIdentity = {
			kind:
				subject.kind === "ServiceAccount"
					? "serviceAccount"
					: subject.kind === "Group"
						? "group"
						: "user",
			name: subject.name,
			namespace: subject.namespace,
		};
		manualIdentity = false;
	}

	function copyEvidence() {
		if (!selected) return;
		void navigator.clipboard?.writeText(JSON.stringify(selected.item, null, 2));
		copied = true;
		setTimeout(() => (copied = false), 1500);
	}

	async function verify() {
		if (
			!data ||
			!activeIdentity.name ||
			!verb.trim() ||
			(targetKind === "resource"
				? !resource.trim()
				: !nonResourceUrl.trim().startsWith("/"))
		) {
			return;
		}
		review = null;
		reviewing = true;
		const identity =
			activeIdentity.kind === "serviceAccount"
				? {
					kind: "serviceAccount" as const,
					name: activeIdentity.name,
					namespace: activeIdentity.namespace || "default",
				}
				: activeIdentity.kind === "group"
					? { kind: "group" as const, group: activeIdentity.name }
					: {
							kind: "user" as const,
							username: activeIdentity.name,
							groups:
								activeIdentity.groups?.length
									? activeIdentity.groups
									: identityDefaults("user", activeIdentity.name),
						};
		const target =
			targetKind === "resource"
				? {
						kind: "resource" as const,
						verb: verb.trim(),
						resource: resource.trim(),
						apiGroup: apiGroup.trim(),
						namespace: namespace.trim() || null,
						subresource: subresource.trim() || undefined,
						name: resourceName.trim() || undefined,
					}
				: {
						kind: "nonResource" as const,
						verb: verb.trim(),
						nonResourceUrl: nonResourceUrl.trim(),
					};
		const request: RbacAccessReviewRequest = {
			clusterContext: data.cluster,
			identity,
			target,
			requestId: "rbac-review",
			cancelScope: "rbac-review",
		};
		const requestFingerprint = fingerprint;
		const version = requestVersion;
		submitted = {
			timestamp: new Date().toISOString(),
			fingerprint: requestFingerprint,
			identity: JSON.stringify(identity),
			target: JSON.stringify(target),
		};
		try {
			const result = await reviewRbacAccess(client, request);
			if (version === requestVersion && requestFingerprint === fingerprint) {
				review = result;
			}
		} catch (error) {
			if (version === requestVersion && requestFingerprint === fingerprint) {
				verifierError = error instanceof Error ? error.message : String(error);
			}
		} finally {
			if (version === requestVersion) reviewing = false;
		}
	}

	function categoryCount(inspection: RbacInspectionSummary, category: RbacView) {
		if (category === "Namespace Access") return inspection.namespaceAccess.length;
		if (category === "Roles") return inspection.roles.length;
		if (category === "Cluster Roles") return inspection.clusterRoles.length;
		if (category === "Bindings") {
			return inspection.roleBindings.length + inspection.clusterRoleBindings.length;
		}
		return inspection.serviceAccounts.length;
	}

	function riskCount(next: RbacRiskBucket) {
		return filterCockpitItems(allItems, next, "").length;
	}

	function queueTags(
		inspection: RbacInspectionSummary,
		entry: RbacCockpitItem,
	): string[] {
		if (entry.category === "Service Accounts") {
			const account = entry.item as ServiceAccountSummary;
			return [
				`${bindingSources(inspection, entry).length} bindings`,
				`${account.secretsCount} secrets`,
				account.automountToken === false ? "token off" : "token posture",
			];
		}
		if (entry.category === "Roles" || entry.category === "Cluster Roles") {
			const role = entry.item as RbacRoleSummary;
			return [
				`${role.rulesCount} rules`,
				role.namespace ?? "cluster-wide",
			];
		}
		if (entry.category === "Bindings") {
			const binding = entry.item as RbacBindingSummary;
			return [
				`${binding.subjects.length} subjects`,
				`${binding.roleRefKind}/${binding.roleRefName}`,
			];
		}
		const namespaceAccess = entry.item as RbacNamespaceAccessSummary;
		return [
			`${namespaceAccess.serviceAccounts} identities`,
			`${namespaceAccess.roleBindings} bindings`,
		];
	}

	function queueSummary(entry: RbacCockpitItem): string {
		if (entry.risks.length) {
			return entry.risks
				.slice(0, 2)
				.map((risk) => risk.label)
				.join(" · ");
		}
		if ("subjects" in entry.item) return subjectListLabel(entry.item.subjects, 2);
		return "No policy heuristic flags for this object.";
	}

	function bindingSources(
		inspection: RbacInspectionSummary,
		entry: RbacCockpitItem,
	): RbacBindingSummary[] {
		const bindings = [...inspection.roleBindings, ...inspection.clusterRoleBindings];
		if (entry.category === "Service Accounts") {
			const account = entry.item as ServiceAccountSummary;
			const groups = new Set([
				"system:serviceaccounts",
				`system:serviceaccounts:${account.namespace}`,
				"system:authenticated",
			]);
			return bindings.filter((binding) =>
				binding.subjects.some(
					(subject) =>
						(subject.kind === "ServiceAccount" &&
							subject.name === account.name &&
							subject.namespace === account.namespace) ||
						(subject.kind === "Group" && groups.has(subject.name)),
				),
			);
		}
		if (entry.category === "Roles" || entry.category === "Cluster Roles") {
			const role = entry.item as RbacRoleSummary;
			return bindings.filter(
				(binding) =>
					binding.roleRefKind === role.kind &&
					binding.roleRefName === role.name &&
					(role.kind !== "Role" || binding.namespace === role.namespace),
			);
		}
		if (entry.category === "Bindings") return [entry.item as RbacBindingSummary];
		return bindings.filter((binding) => binding.namespace === entry.namespace);
	}

	function selectedReach(): string {
		if (!observed || observed.grants.length === 0) return observed?.unknown ? "Unknown" : "No grants";
		if (observed.grants.some((grant) => grant.scope === "cluster")) return "Cluster-wide";
		const namespaces = new Set(
			observed.grants.map((grant) => grant.namespace).filter(Boolean),
		);
		return `${namespaces.size} namespace${namespaces.size === 1 ? "" : "s"}`;
	}

	function identityLabel(identity: InspectorIdentity): string {
		if (!identity.name) return "No identity selected";
		if (identity.kind === "serviceAccount") {
			return `system:serviceaccount:${identity.namespace ?? "default"}:${identity.name}`;
		}
		return identity.kind === "group"
			? `Group/${identity.name}`
			: `User/${identity.name}`;
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

	function policyShapeLabel(entry: RbacCockpitItem): string {
		return entry.category === "Service Accounts" ? "Token posture" : "Policy shape";
	}

	function policyShapeValue(entry: RbacCockpitItem): string {
		if (entry.category === "Service Accounts") {
			const account = entry.item as ServiceAccountSummary;
			return account.automountToken === true
				? "Automount on"
				: account.automountToken === false
					? "Automount off"
					: "Not reported";
		}
		if (entry.category === "Roles" || entry.category === "Cluster Roles") {
			return `${(entry.item as RbacRoleSummary).rulesCount} rules`;
		}
		if (entry.category === "Bindings") {
			return `${(entry.item as RbacBindingSummary).subjects.length} subjects`;
		}
		return `${(entry.item as RbacNamespaceAccessSummary).roles} roles`;
	}

	function riskRailClass(risks: RbacRiskIndicator[]): string {
		if (risks.some((risk) => risk.level === "high")) return "border-l-destructive";
		if (risks.some((risk) => risk.level === "medium")) return "border-l-amber-500";
		if (risks.some((risk) => risk.level === "low")) return "border-l-sky-500";
		if (risks.some((risk) => risk.level === "unknown")) return "border-l-muted-foreground";
		return "border-l-emerald-500";
	}

	function riskBadgeClass(risks: RbacRiskIndicator[]): string {
		if (risks.some((risk) => risk.level === "high")) {
			return "border-destructive/35 bg-destructive/10 text-destructive";
		}
		if (risks.some((risk) => risk.level === "medium")) {
			return "border-amber-500/35 bg-amber-500/10 text-amber-600 dark:text-amber-300";
		}
		if (risks.some((risk) => risk.level === "unknown")) {
			return "border-muted-foreground/35 bg-muted text-muted-foreground";
		}
		return "";
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

	function uniqueGroups(value: string): string[] {
		return [
			...new Set(
				value
					.split(",")
					.map((group) => group.trim())
					.filter(Boolean),
			),
		];
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
	{#if inspection}
		<div class="flex min-h-0 flex-col gap-3">
			<div class="flex flex-wrap items-center gap-2">
				<Badge variant="outline" class="border-emerald-500/35 bg-emerald-500/10 text-emerald-600 dark:text-emerald-300">
					{completeCoverage === coverage.length ? "Coverage complete" : "Coverage partial"}
				</Badge>
				<Badge variant="outline">Entire cluster</Badge>
			</div>

			<Card size="sm" elevation="flat">
				<CardContent class="grid gap-3 p-0 lg:grid-cols-[minmax(14rem,1.1fr)_minmax(30rem,2.4fr)_auto] lg:items-stretch">
					<div class="px-3 py-2.5">
						<p class="text-sm font-semibold">Access posture</p>
						<p class="mt-0.5 text-xs text-muted-foreground">
							{inspection.cluster} · {completeCoverage}/{coverage.length || 5} RBAC families · {freshnessLabel(inspection.refreshedAt)}
						</p>
					</div>
					<div class="grid grid-cols-2 border-y border-border sm:grid-cols-5 lg:border-x lg:border-y-0">
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
						<Button size="sm" onclick={() => beginIdentity()}>
							<ShieldCheck data-icon /> Investigate identity
						</Button>
					</div>
				</CardContent>
			</Card>

			<Card size="sm" elevation="flat" class="gap-0 py-0">
				<CardContent class="flex flex-col gap-2 px-2 py-2 xl:flex-row xl:items-center">
					<div class="flex min-w-0 items-center gap-1 overflow-x-auto" aria-label="RBAC categories">
						<span class="shrink-0 px-1 text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Explore</span>
						{#each categories as category}
							<Button
								size="sm"
								variant={view === category ? "secondary" : "ghost"}
								class={cnfast("shrink-0", view === category && "text-foreground")}
								aria-pressed={view === category}
								onclick={() => setView(category)}
							>
								{category}
								<Badge variant="outline" class="h-4 px-1.5 tabular-nums">{categoryCount(inspection, category)}</Badge>
							</Button>
						{/each}
					</div>
					<div class="flex min-w-0 flex-1 flex-wrap items-center gap-1 xl:justify-end">
						<span class="px-1 text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Risk</span>
						{#each riskFilters as [riskBucket, label]}
							<Button
								size="xs"
								variant={bucket === riskBucket ? "default" : "outline"}
								aria-pressed={bucket === riskBucket}
								onclick={() => setBucket(riskBucket)}
							>
								{label} <span class="tabular-nums opacity-75">{riskCount(riskBucket)}</span>
							</Button>
						{/each}
						<div class="relative min-w-48 flex-1 xl:max-w-72">
							<Search class="pointer-events-none absolute left-2 top-1.5 size-3.5 text-muted-foreground" />
							<Input class="pl-7" bind:value={search} placeholder={`Search ${view.toLowerCase()}…`} />
						</div>
					</div>
				</CardContent>
			</Card>

			{#if handoffPromoted}
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
				<Card size="sm" elevation="flat" class="min-h-0 gap-0 py-0">
					<div class="flex items-start gap-3 border-b border-border px-3 py-3">
						<div class="min-w-0 flex-1">
							<p class="text-sm font-semibold">{view === "Service Accounts" ? "Service account review queue" : `${view} review queue`}</p>
							<p class="mt-0.5 text-xs text-muted-foreground">Highest risk · most findings · namespace and name</p>
						</div>
						<Badge variant="secondary" class="tabular-nums">{items.length}</Badge>
					</div>
					<div class="min-h-0 flex-1 space-y-2 overflow-y-auto p-2">
						{#each items.slice(0, visible) as item (item.key)}
							<button
								class={cnfast(
									"w-full rounded-md border border-l-[3px] bg-card p-3 text-left transition-colors hover:bg-muted/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring active:scale-[.995] motion-reduce:transition-none",
									riskRailClass(item.risks),
									selected?.key === item.key && "border-primary/70 bg-primary/5 ring-1 ring-primary/40",
								)}
								aria-pressed={selected?.key === item.key}
								onclick={() => select(item.key)}
							>
								<div class="flex items-start gap-2">
									<div class="min-w-0 flex-1">
										<p class="truncate text-sm font-semibold">{item.kind}/{item.name}</p>
										<p class="truncate text-xs text-muted-foreground">{item.namespace ?? "cluster scope"}</p>
									</div>
									<Badge variant="outline" class={riskBadgeClass(item.risks)}>{riskSummaryLabel(item.risks)}</Badge>
								</div>
								<div class="mt-2 flex flex-wrap gap-1">
									{#each queueTags(inspection, item) as tag}
										<Badge variant="outline" class="h-4 rounded-sm px-1.5 font-normal">{tag}</Badge>
									{/each}
								</div>
								<p class="mt-2 truncate text-xs text-muted-foreground">{queueSummary(item)}</p>
							</button>
						{:else}
							<div class="p-6 text-center text-sm text-muted-foreground">No matching RBAC objects.</div>
						{/each}
						{#if visible < items.length}
							<Button class="w-full" variant="outline" size="sm" onclick={() => (visible += 50)}>Load 50 more</Button>
						{/if}
					</div>
				</Card>

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
						{#if selected}<Badge variant="outline" class={riskBadgeClass(selected.risks)}>{riskSummaryLabel(selected.risks)}</Badge>{/if}
						<Button size="sm" variant="outline" onclick={copyEvidence} disabled={!selected}>
							{#if copied}<Check data-icon /> Copied{:else}<Clipboard data-icon /> Copy evidence{/if}
						</Button>
					</div>

					<CardContent class="min-h-0 flex-1 overflow-y-auto p-3">
						{#if selected}
							<div class="grid gap-3 lg:grid-cols-[minmax(0,1.45fr)_minmax(17rem,.75fr)]">
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
												{policyShapeLabel(selected)}
											</p>
											<p class="mt-1 text-sm font-semibold">
												{policyShapeValue(selected)}
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
													<button class="flex w-full items-center gap-2 py-2 text-left text-xs hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring" onclick={() => inspectSubject(subject)}>
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
													<p class="font-mono text-xs">{grant.verbs.join(", ")} · {grant.apiGroups.join(", ") || "core"}/{grant.resources.join(", ") || grant.nonResourceUrls.join(", ")}</p>
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

								<aside class="min-w-0 space-y-3 lg:sticky lg:top-0 lg:self-start">
									<section class={cnfast("rounded-md border bg-card", handoffPromoted && "border-amber-500/50 ring-1 ring-amber-500/20")}>
										<div class="border-b border-border px-3 py-2.5">
											<div class="flex items-center gap-2">
												<ShieldCheck class="size-4 text-primary" />
												<p class="text-sm font-semibold">Verify exact action</p>
											</div>
											<p class="mt-1 text-xs text-muted-foreground">Live SubjectAccessReview. Manual only.</p>
										</div>
										<div class="space-y-2.5 p-3">
											{#if !manualIdentity && activeIdentity.name}
												<div class="rounded-sm border bg-muted/25 p-2">
													<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Identity</p>
													<p class="mt-1 break-all text-xs font-medium">{identityLabel(activeIdentity)}</p>
													<Button class="mt-2" size="xs" variant="outline" onclick={() => beginIdentity(activeIdentity.kind)}>Change identity</Button>
												</div>
											{:else}
												<div class="flex flex-wrap gap-1">
													{#each [["serviceAccount", "ServiceAccount"], ["user", "User"], ["group", "Group"]] as identityOption}
														<Button size="xs" variant={identityKind === identityOption[0] ? "default" : "outline"} onclick={() => beginIdentity(identityOption[0] as "serviceAccount" | "user" | "group")}>{identityOption[1]}</Button>
													{/each}
												</div>
												<Input bind:value={identityName} aria-label="Identity name" placeholder={identityKind === "group" ? "Group" : "Identity name"} />
												{#if identityKind === "serviceAccount"}
													<Input bind:value={identityNamespace} aria-label="ServiceAccount namespace" placeholder="Namespace" />
												{:else if identityKind === "user"}
													<Input bind:value={identityGroups} aria-label="Groups" placeholder="Groups, comma-separated" />
												{/if}
											{/if}

											<div class="flex gap-1">
												<Button size="xs" variant={targetKind === "resource" ? "default" : "outline"} onclick={() => (targetKind = "resource")}>Resource</Button>
												<Button size="xs" variant={targetKind === "nonResource" ? "default" : "outline"} onclick={() => (targetKind = "nonResource")}>Non-resource</Button>
											</div>
											<div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-1 2xl:grid-cols-2">
												<Input bind:value={verb} aria-label="Verb" placeholder="Verb" />
												{#if targetKind === "resource"}
													<Input bind:value={resource} aria-label="Resource" placeholder="Resource" />
													<Input bind:value={apiGroup} aria-label="API group, blank for core" placeholder="API group (core)" />
													<Input bind:value={namespace} aria-label="Namespace, blank for cluster" placeholder="Namespace (cluster if blank)" />
													<Input bind:value={subresource} aria-label="Subresource, optional" placeholder="Subresource" />
													<Input bind:value={resourceName} aria-label="Resource name, optional" placeholder="Resource name" />
												{:else}
													<Input bind:value={nonResourceUrl} aria-label="Non-resource URL" placeholder="/path" />
												{/if}
											</div>
											<Button
												class="w-full"
												size="sm"
												disabled={reviewing || !activeIdentity.name || !verb.trim() || (targetKind === "resource" ? !resource.trim() : !nonResourceUrl.trim().startsWith("/"))}
												onclick={verify}
											>
												<ShieldCheck data-icon /> {reviewing ? "Verifying…" : "Verify access"}
											</Button>
											<p class="text-[0.625rem] text-muted-foreground">Result is never inferred from observed grants.</p>
										</div>
									</section>

									{#if review || verifierError || submitted}
										<section class="rounded-md border p-3">
											<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Latest verification</p>
											{#if review}
												<p class={cnfast("mt-2 text-sm font-semibold capitalize", review.outcome === "allowed" && "text-emerald-600 dark:text-emerald-300", review.outcome === "denied" && "text-destructive", review.outcome === "noOpinion" && "text-amber-600 dark:text-amber-300")}>{review.outcome === "noOpinion" ? "No opinion" : review.outcome}</p>
												{#if review.reason}<p class="mt-1 text-xs text-muted-foreground">{review.reason}</p>{/if}
												{#if review.evaluationError}<p class="mt-1 text-xs text-destructive">{review.evaluationError}</p>{/if}
											{/if}
											{#if verifierError}<p class="mt-2 text-xs text-destructive">Verifier unavailable · {verifierError}</p>{/if}
											{#if submitted}<p class="mt-2 break-words text-[0.625rem] text-muted-foreground">{inspection.cluster} · {submitted.timestamp}<br />{submitted.identity}<br />{submitted.target}</p>{/if}
										</section>
									{/if}

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
						{:else}
							<div class="flex min-h-64 items-center justify-center text-sm text-muted-foreground">Select an RBAC object to inspect evidence.</div>
						{/if}
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
