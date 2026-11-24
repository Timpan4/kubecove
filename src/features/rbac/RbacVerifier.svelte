<script lang="ts">
	import { ShieldCheck } from "lucide-svelte";
	import TimestampText from "@/components/TimestampText.svelte";
	import { Button, Input } from "@/components/ui/svelte";
	import { cnfast } from "@/lib/utils";
	import type { InspectorIdentity } from "./observedPermissions";
	import { identityLabel, type rbacVerifier } from "./rbacVerifier.svelte";

	let { model, cluster }: { model: ReturnType<typeof rbacVerifier>; cluster: string } = $props();
	const activeIdentity = $derived(model.activeIdentity);
	const accessReview = $derived(model.accessReview);
	const identityOptions: Array<[InspectorIdentity["kind"], string]> = [
		["serviceAccount", "ServiceAccount"],
		["user", "User"],
		["group", "Group"],
	];

</script>

<section class={cnfast("rounded-md border bg-card", model.handoffPromoted && "border-amber-500/50 ring-1 ring-amber-500/20")}>
	<div class="border-b border-border px-3 py-2.5">
		<div class="flex items-center gap-2">
			<ShieldCheck class="size-4 text-primary" />
			<p class="text-sm font-semibold">Verify exact action</p>
		</div>
		<p class="mt-1 text-xs text-muted-foreground">Live SubjectAccessReview. Manual only.</p>
	</div>
	<div class="space-y-2.5 p-3">
		{#if !model.manual && activeIdentity.name}
			<div class="rounded-sm border bg-muted/25 p-2">
				<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Identity</p>
				<p class="mt-1 break-all text-xs font-medium">{identityLabel(activeIdentity)}</p>
				<Button class="mt-2" size="xs" variant="outline" onclick={() => model.beginIdentity(activeIdentity.kind)}>Change identity</Button>
			</div>
		{:else}
			<div class="flex flex-wrap gap-1">
			{#each identityOptions as identityOption}
				<Button size="xs" variant={model.identityKind === identityOption[0] ? "default" : "outline"} onclick={() => model.beginIdentity(identityOption[0])}>{identityOption[1]}</Button>
				{/each}
			</div>
			<Input bind:value={model.identityName} oninput={model.editManualIdentity} aria-label="Identity name" placeholder={model.identityKind === "group" ? "Group" : "Identity name"} />
			{#if model.identityKind === "serviceAccount"}
				<Input bind:value={model.identityNamespace} oninput={model.editManualIdentity} aria-label="ServiceAccount namespace" placeholder="Namespace" />
			{:else if model.identityKind === "user"}
				<Input bind:value={model.identityGroups} oninput={model.editManualIdentity} aria-label="Groups" placeholder="Groups, comma-separated" />
			{/if}
		{/if}

		<div class="flex gap-1">
			<Button size="xs" variant={model.targetKind === "resource" ? "default" : "outline"} onclick={() => (model.targetKind = "resource")}>Resource</Button>
			<Button size="xs" variant={model.targetKind === "nonResource" ? "default" : "outline"} onclick={() => (model.targetKind = "nonResource")}>Non-resource</Button>
		</div>
		<div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-1 2xl:grid-cols-2">
			<Input bind:value={model.verb} aria-label="Verb" placeholder="Verb" />
			{#if model.targetKind === "resource"}
				<Input bind:value={model.resource} aria-label="Resource" placeholder="Resource" />
				<Input bind:value={model.apiGroup} aria-label="API group, blank for core" placeholder="API group (core)" />
				<Input bind:value={model.namespace} aria-label="Namespace, blank for cluster" placeholder="Namespace (cluster if blank)" />
				<Input bind:value={model.subresource} aria-label="Subresource, optional" placeholder="Subresource" />
				<Input bind:value={model.resourceName} aria-label="Resource name, optional" placeholder="Resource name" />
			{:else}
				<Input bind:value={model.nonResourceUrl} aria-label="Non-resource URL" placeholder="/path" />
			{/if}
		</div>
		<Button
			class="w-full"
			size="sm"
			disabled={model.reviewing || !model.canVerify}
			onclick={model.verify}
		>
			<ShieldCheck data-icon /> {model.reviewing ? "Verifying…" : "Verify access"}
		</Button>
		<p class="text-[0.625rem] text-muted-foreground">Result is never inferred from observed grants.</p>
	</div>
</section>

{#if accessReview.kind !== "none"}
	{@const submitted = accessReview.submitted}
	{@const review = accessReview.kind === "done" ? accessReview.result : null}
	<section class="rounded-md border p-3">
		<p class="text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Latest verification</p>
		{#if review}
			<p class={cnfast("mt-2 text-sm font-semibold capitalize", review.outcome === "allowed" && "text-emerald-600 dark:text-emerald-300", review.outcome === "denied" && "text-destructive", review.outcome === "noOpinion" && "text-amber-600 dark:text-amber-300")}>{review.outcome === "noOpinion" ? "No opinion" : review.outcome}</p>
			{#if review.reason}<p class="mt-1 text-xs text-muted-foreground">{review.reason}</p>{/if}
			{#if review.evaluationError}<p class="mt-1 text-xs text-destructive">{review.evaluationError}</p>{/if}
		{/if}
		{#if accessReview.kind === "failed"}<p class="mt-2 text-xs text-destructive">Verifier unavailable · {accessReview.message}</p>{/if}
		<p class="mt-2 break-words text-[0.625rem] text-muted-foreground">{cluster} · <TimestampText value={submitted.timestamp} precision="second" /><br />{submitted.identity}<br />{submitted.target}</p>
	</section>
{/if}
