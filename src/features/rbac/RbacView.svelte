<script lang="ts">
	import { Check, Clipboard, KeyRound, RefreshCw, Search, ShieldCheck } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import SurfaceFrame from "@/components/SurfaceFrame.svelte";
	import { Badge, Button, Card, CardContent, CardDescription, CardHeader, CardTitle, Input } from "@/components/ui/svelte";
	import type { RbacAccessReviewRequest, RbacAccessReviewResult, RbacInspectionSummary } from "@/lib/types";
	import { reviewRbacAccess } from "@/lib/tauri";
	import { createTauriClient } from "@/lib/tauri";
	import { cockpitItems, filterCockpitItems, ruleText, selectedCockpitItem, type RbacCockpitState, type RbacRiskBucket } from "./cockpitModel";
	import type { RbacVerifierHandoff } from "./handoff";
	import { riskSummaryLabel, subjectListLabel } from "./risk";
	import type { RbacView } from "./surfaceModel";
	import { identityDefaults, observedPermissions } from "./observedPermissions";

	let { query, view, warningSummary, initialState, onStateChange, verifierHandoff, onVerifierHandoffConsumed, onVerifierReturn, verifierReturnLabel }: {
		query: { data?: RbacInspectionSummary; isPending: boolean; isError: boolean; error: unknown; refetch?: () => void };
		view: RbacView;
		warningSummary: (warnings: string[]) => string;
		initialState?: RbacCockpitState;
		onStateChange?: (state: RbacCockpitState) => void;
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
	let drillIdentity = $state<{ kind: "serviceAccount" | "user" | "group"; name: string; namespace?: string } | null>(null);
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
	let submitted = $state<{ timestamp: string; fingerprint: string; identity: string; target: string } | null>(null);
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
		drillIdentity = handoff.identity?.kind === "serviceAccount"
			? { kind: "serviceAccount", name: handoff.identity.name, namespace: handoff.identity.namespace }
			: handoff.identity?.kind === "group"
				? { kind: "group", name: handoff.identity.group }
				: handoff.identity?.kind === "user"
					? { kind: "user", name: handoff.identity.username }
					: null;
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
	const activeIdentity = $derived(drillIdentity ?? { kind: identityKind, name: identityName.trim(), namespace: identityNamespace.trim(), groups: uniqueGroups(identityGroups) });
	const observed = $derived(data && activeIdentity.name ? observedPermissions(data, activeIdentity) : null);
	const fingerprint = $derived(JSON.stringify({ cluster: data?.cluster, refreshedAt: data?.refreshedAt, activeIdentity, targetKind, verb, apiGroup, resource, namespace, subresource, resourceName, nonResourceUrl }));

	$effect(() => { fingerprint; review = null; verifierError = ""; submitted = null; requestVersion += 1; reviewing = false; });

	function select(key: string) {
		selectedKey = key;
		onStateChange?.({ riskBucket: bucket, selectedObjectKey: key });
	}
	function setBucket(next: RbacRiskBucket) {
		bucket = next; visible = 50;
		onStateChange?.({ riskBucket: next, selectedObjectKey: selectedKey });
	}
	function beginIdentity(kind: "serviceAccount" | "user" | "group" = "user") {
		drillIdentity = null; identityKind = kind; identityName = ""; identityNamespace = "default";
		identityGroups = identityDefaults(kind).join(", ");
	}
	function copyEvidence() {
		if (!selected) return;
		navigator.clipboard?.writeText(JSON.stringify(selected.item, null, 2));
		copied = true;
		setTimeout(() => copied = false, 1500);
	}
	async function verify() {
		if (!data || !activeIdentity.name || !verb.trim() || (targetKind === "resource" ? !resource.trim() : !nonResourceUrl.trim().startsWith("/"))) return;
		review = null; reviewing = true;
		const identity = activeIdentity.kind === "serviceAccount"
			? { kind: "serviceAccount" as const, name: activeIdentity.name, namespace: activeIdentity.namespace || "default" }
			: activeIdentity.kind === "group"
				? { kind: "group" as const, group: activeIdentity.name }
				: { kind: "user" as const, username: activeIdentity.name, groups: "groups" in activeIdentity && activeIdentity.groups.length ? activeIdentity.groups : identityDefaults("user", activeIdentity.name) };
		const target = targetKind === "resource"
			? { kind: "resource" as const, verb: verb.trim(), resource: resource.trim(), apiGroup: apiGroup.trim(), namespace: namespace.trim() || null, subresource: subresource.trim() || undefined, name: resourceName.trim() || undefined }
			: { kind: "nonResource" as const, verb: verb.trim(), nonResourceUrl: nonResourceUrl.trim() };
		const request: RbacAccessReviewRequest = { clusterContext: data.cluster, identity, target, requestId: `rbac-review-${requestVersion}`, cancelScope: "rbac-review" };
		const requestFingerprint = fingerprint;
		const version = requestVersion;
		submitted = { timestamp: new Date().toISOString(), fingerprint: requestFingerprint, identity: JSON.stringify(identity), target: JSON.stringify(target) };
		try { const result = await reviewRbacAccess(client, request); if (version === requestVersion && requestFingerprint === fingerprint) review = result; }
		catch (error) { if (version === requestVersion && requestFingerprint === fingerprint) verifierError = error instanceof Error ? error.message : String(error); }
		finally { if (version === requestVersion) reviewing = false; }
	}
	function uniqueGroups(value: string): string[] { return [...new Set(value.split(",").map((group) => group.trim()).filter(Boolean))]; }
</script>

<SurfaceFrame icon={KeyRound} title="RBAC Cockpit" {query} wide errorLabel="RBAC inspection unavailable">
	{@const inspection = data}
	{#if inspection}
		<div class="flex flex-col gap-3">
			<Card size="sm" elevation="flat"><CardContent class="flex flex-wrap items-center gap-3 p-3"><div class="mr-auto"><p class="font-medium">{inspection.cluster}</p><p class="text-xs text-muted-foreground">Full-cluster policy inspection · {inspection.refreshedAt ? new Date(inspection.refreshedAt).toLocaleString() : "freshness unavailable"}</p></div><Badge variant="outline">{coverage.filter((item) => item.status === "complete").length}/{coverage.length || 5} families complete</Badge><Button size="sm" variant="outline" onclick={() => query.refetch?.()}><RefreshCw data-icon /> Refresh</Button><Button size="sm" onclick={() => beginIdentity()}><ShieldCheck data-icon /> Investigate identity</Button></CardContent></Card>
			<div class="grid gap-3 xl:grid-cols-5">
				<Card size="sm" elevation="flat" class="xl:col-span-2"><CardHeader><CardTitle>{view}</CardTitle><CardDescription>Risk-sorted queue. Observed grants are policy-derived, not an authorization verdict.</CardDescription></CardHeader><CardContent class="space-y-3"><div class="flex flex-wrap gap-1">{#each [["all", "All"], ["high", "High"], ["medium", "Medium"], ["low", "Low"], ["none", "No flags"], ["unknown", "Unknown"]] as pair}<Button size="xs" variant={bucket === pair[0] ? "default" : "outline"} onclick={() => setBucket(pair[0] as RbacRiskBucket)}>{pair[1]}</Button>{/each}</div><div class="relative"><Search class="pointer-events-none absolute left-2 top-2.5 size-4 text-muted-foreground" /><Input class="pl-8" bind:value={search} placeholder="Search names, roles, subjects…" /></div><div class="max-h-[62vh] space-y-2 overflow-y-auto pr-1">{#each items.slice(0, visible) as item (item.key)}<button class="w-full rounded-md border p-3 text-left transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring active:scale-[.99] motion-reduce:transition-none" class:bg-muted={selected?.key === item.key} onclick={() => select(item.key)}><div class="flex items-start gap-2"><div class="min-w-0 flex-1"><p class="truncate text-sm font-medium">{item.kind}/{item.name}</p><p class="truncate text-xs text-muted-foreground">{item.namespace ?? "cluster scope"}{"roleRefName" in item.item ? ` · ${item.item.roleRefKind}/${item.item.roleRefName}` : ""}</p></div><Badge variant="outline">{riskSummaryLabel(item.risks)}</Badge></div>{#if "subjects" in item.item}<p class="mt-2 truncate text-xs text-muted-foreground">{subjectListLabel(item.item.subjects, 2)}</p>{/if}</button>{:else}<p class="p-4 text-sm text-muted-foreground">No matching RBAC objects.</p>{/each}{#if visible < items.length}<Button class="w-full" variant="outline" size="sm" onclick={() => visible += 50}>Load 50 more</Button>{/if}</div></CardContent></Card>
				<Card size="sm" elevation="flat" class="xl:col-span-3"><CardHeader><div class="flex items-start gap-3"><div class="min-w-0 flex-1"><CardTitle>{selected ? `${selected.kind}/${selected.name}` : "Select an RBAC object"}</CardTitle><CardDescription>{selected?.namespace ?? "Cluster scope"} · exact evidence and policy findings</CardDescription></div><Button size="sm" variant="outline" onclick={copyEvidence} disabled={!selected}>{#if copied}<Check data-icon /> Copied{:else}<Clipboard data-icon /> Copy evidence{/if}</Button></div></CardHeader><CardContent class="space-y-4">{#if selected}<section class="rounded-md border p-3"><p class="text-xs font-medium text-muted-foreground">FINDINGS</p>{#each selected.risks as risk}<p class="mt-2 text-sm"><strong>{risk.level.toUpperCase()} · {risk.label}</strong><span class="text-muted-foreground"> — {risk.reason}</span></p>{:else}<p class="mt-2 text-sm text-muted-foreground">No policy heuristic flags for this object.</p>{/each}</section>{#if "rules" in selected.item}<section class="rounded-md border p-3"><p class="text-xs font-medium text-muted-foreground">EXACT RULES</p>{#each selected.item.rules as rule}<p class="mt-2 break-words font-mono text-xs">{ruleText(rule)}</p>{:else}<p class="mt-2 text-sm text-muted-foreground">No rules.</p>{/each}</section>{/if}{#if "automountToken" in selected.item}<section class="rounded-md border p-3"><p class="text-xs font-medium text-muted-foreground">SERVICEACCOUNT TOKEN POSTURE</p><p class="mt-2 text-sm">Automount token: {selected.item.automountToken === true ? "enabled" : selected.item.automountToken === false ? "disabled" : "not reported"}</p><p class="text-sm text-muted-foreground">Secrets: {selected.item.secretsCount} · Image pull secrets: {selected.item.imagePullSecretsCount}</p></section>{/if}{#if "subjects" in selected.item}<section class="rounded-md border p-3"><p class="text-xs font-medium text-muted-foreground">SUBJECTS · {selected.item.roleRefKind}/{selected.item.roleRefName}</p>{#each selected.item.subjects as subject}<button class="mt-2 block text-left text-sm underline-offset-2 hover:underline focus-visible:ring-2 focus-visible:ring-ring" onclick={() => drillIdentity = { kind: subject.kind === "ServiceAccount" ? "serviceAccount" : subject.kind === "Group" ? "group" : "user", name: subject.name, namespace: subject.namespace }}>{subject.kind}/{subject.namespace ? `${subject.namespace}/` : ""}{subject.name}</button>{:else}<p class="mt-2 text-sm text-muted-foreground">No subjects.</p>{/each}</section>{/if}{/if}<section class="rounded-md border p-3"><p class="text-xs font-medium text-muted-foreground">OBSERVED GRANTS</p>{#if observed}<p class="mt-2 text-xs text-muted-foreground">Observed RBAC grants — policy-derived, not an authorization verdict.</p>{#if observed.unknown}<p class="mt-2 text-sm text-muted-foreground">Unknown · {observed.reason}</p>{/if}{#each observed.grants as grant}<p class="mt-2 break-words font-mono text-xs">{grant.scope}{grant.namespace ? `/${grant.namespace}` : ""} · {grant.verbs.join(",")} · {grant.resources.join(",")} · {grant.roles.join(", ")} · {grant.bindings.join(", ")}</p>{:else}<p class="mt-2 text-sm text-muted-foreground">Choose an identity to inspect grants.</p>{/each}{/if}</section><section class="sticky bottom-0 rounded-md border bg-card p-3 shadow-sm"><div class="flex items-center gap-2"><p class="mr-auto text-xs font-medium text-muted-foreground">PERMISSION VERIFICATION</p>{#if onVerifierReturn}<Button size="xs" variant="outline" onclick={onVerifierReturn}>{verifierReturnLabel ?? "Return"}</Button>{/if}</div><p class="mt-1 text-xs text-muted-foreground">Live SubjectAccessReview. Manual only; result is not inferred from observed grants.</p><div class="mt-2 flex gap-1"><Button size="xs" variant={identityKind === "serviceAccount" ? "default" : "outline"} onclick={() => beginIdentity("serviceAccount")}>ServiceAccount</Button><Button size="xs" variant={identityKind === "user" ? "default" : "outline"} onclick={() => beginIdentity("user")}>User</Button><Button size="xs" variant={identityKind === "group" ? "default" : "outline"} onclick={() => beginIdentity("group")}>Group</Button></div><div class="mt-2 grid gap-2 md:grid-cols-3"><Input bind:value={identityName} aria-label="Identity name" placeholder={identityKind === "group" ? "Group" : "Identity"} />{#if identityKind === "serviceAccount"}<Input bind:value={identityNamespace} aria-label="ServiceAccount namespace" placeholder="Namespace" />{:else if identityKind === "user"}<Input class="md:col-span-2" bind:value={identityGroups} aria-label="Groups" placeholder="Groups, comma-separated" />{/if}</div><div class="mt-2 flex gap-1"><Button size="xs" variant={targetKind === "resource" ? "default" : "outline"} onclick={() => targetKind = "resource"}>Resource</Button><Button size="xs" variant={targetKind === "nonResource" ? "default" : "outline"} onclick={() => targetKind = "nonResource"}>Non-resource</Button></div><div class="mt-2 grid gap-2 md:grid-cols-3"><Input bind:value={verb} aria-label="Verb" />{#if targetKind === "resource"}<Input bind:value={apiGroup} aria-label="API group, blank for core" placeholder="API group (core)" /><Input bind:value={resource} aria-label="Resource" /><Input bind:value={namespace} aria-label="Namespace, blank for cluster" /><Input bind:value={subresource} aria-label="Subresource, optional" placeholder="Subresource" /><Input bind:value={resourceName} aria-label="Resource name, optional" placeholder="Resource name" />{:else}<Input class="md:col-span-2" bind:value={nonResourceUrl} aria-label="Non-resource URL" />{/if}</div><Button class="mt-2" size="sm" disabled={reviewing || !activeIdentity.name || !verb.trim() || (targetKind === "resource" ? !resource.trim() : !nonResourceUrl.trim().startsWith("/"))} onclick={verify}><ShieldCheck data-icon /> {reviewing ? "Verifying…" : "Verify access"}</Button>{#if submitted}<p class="mt-2 break-words text-xs text-muted-foreground">Submitted {submitted.timestamp} · {submitted.identity} · {submitted.target}</p>{/if}{#if review}<p class="mt-2 text-sm"><strong>{review.outcome}</strong>{review.reason ? ` · ${review.reason}` : ""}{review.evaluationError ? ` · ${review.evaluationError}` : ""}</p>{/if}{#if verifierError}<p class="mt-2 text-sm text-destructive">Verifier unavailable · {verifierError}</p>{/if}</section></CardContent></Card>
			</div>
			{#if inspection.warnings.length > 0}<FriendlyError mode="compact" error={warningSummary(inspection.warnings)} context={{ operation: "resourcesLoad", fallbackTitle: "Partial RBAC data", partial: true }} />{/if}
		</div>
	{/if}
</SurfaceFrame>
