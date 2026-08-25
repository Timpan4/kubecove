<script lang="ts">
	import { Button, Checkbox, Input, Label } from "@/components/ui/svelte";
	import OperationScopeCard from "@/components/OperationScopeCard.svelte";
	import {
		deleteResource,
		previewDeleteResource,
		previewRolloutRestart,
		previewScaleWorkload,
		rolloutRestart,
		scaleWorkload,
		type TauriClient,
	} from "@/lib/tauri";
	import type { ResourceSummary } from "@/lib/types";
	import {
		guardedOperationBlocker,
		guardedOperations,
		type GuardedOperationId,
	} from "./operations-model";
	import { getErrorMessage } from "./helpers";

	let { client, resource, kubeconfigSourceKey }: { client: TauriClient; resource: ResourceSummary; kubeconfigSourceKey?: string } = $props();
	let replicas = $state("1");
	let confirmed = $state(false);
	let preview = $state("");
	let previewFingerprint = $state("");
	let result = $state("");
	let error = $state<unknown>(null);
	let busy = $state(false);
	const target = $derived({ clusterContext: resource.cluster, namespace: resource.namespace, kind: resource.kind, name: resource.name });
	const operations = $derived(guardedOperations(resource));
	const errorBlocker = $derived(error ? guardedOperationBlocker(error) : null);

	function clearFeedback() { preview = ""; previewFingerprint = ""; result = ""; error = null; }
	function reset() { confirmed = false; clearFeedback(); }
	function fingerprint(action: GuardedOperationId) {
		return JSON.stringify({ action, target, replicas: action === "scale" ? Number(replicas) : undefined });
	}
	async function run(action: GuardedOperationId, execute: boolean) {
		const requestFingerprint = fingerprint(action);
		if (execute && (!confirmed || previewFingerprint !== requestFingerprint)) {
			error = "Preview this exact operation before confirming it.";
			return;
		}
		if (!execute) {
			preview = "";
			previewFingerprint = "";
			confirmed = false;
		}
		result = ""; error = null; busy = true;
		try {
			const request = { ...target, confirmed: execute && confirmed, kubeconfigEnvVar: kubeconfigSourceKey };
			const response = action === "scale"
				? await (execute ? scaleWorkload(client, { ...request, replicas: Number(replicas) }) : previewScaleWorkload(client, { ...request, replicas: Number(replicas) }))
				: action === "restart"
					? await (execute ? rolloutRestart(client, request) : previewRolloutRestart(client, request))
					: await (execute ? deleteResource(client, request) : previewDeleteResource(client, request));
			if (execute) {
				result = response.effect;
				confirmed = false;
				preview = "";
				previewFingerprint = "";
			} else {
				preview = response.effect;
				previewFingerprint = requestFingerprint;
			}
		} catch (caught) { error = caught; }
		finally { busy = false; }
	}
</script>

<div class="space-y-4 rounded-md border bg-muted/20 p-4 text-sm">
	<div><h3 class="font-medium">Guarded operations</h3><p class="mt-1 text-xs text-muted-foreground">Each action targets only the exact resource shown below. Kubernetes permission is checked during preview; any returned error appears here.</p></div>
	{#each operations.available as operation (operation.id)}
		<div class="space-y-2 rounded border bg-background p-3">
			<div class="text-xs font-semibold">{operation.label}</div>
			<OperationScopeCard
				context={target.clusterContext}
				namespace={target.namespace}
				kind={target.kind}
				resource={target.name}
				operationScope={operation.scope}
			/>
			{#if operation.requiresReplicas}<Label for="operation-replicas">Desired replicas</Label><Input id="operation-replicas" type="number" min="0" bind:value={replicas} oninput={reset} />{/if}
			<div class="flex gap-2">
				<Button variant="outline" disabled={busy} onclick={() => run(operation.id, false)}>{operation.previewLabel}</Button>
				<Button variant={operation.destructive ? "destructive" : "default"} disabled={busy || !confirmed || previewFingerprint !== fingerprint(operation.id)} onclick={() => run(operation.id, true)}>{operation.executeLabel}</Button>
			</div>
		</div>
	{/each}
	{#if operations.blocker}<p class="text-xs text-muted-foreground">{operations.blocker}</p>{/if}
	{#if operations.available.length > 0}<Label class="gap-2 rounded border bg-background p-3 text-xs text-muted-foreground"><Checkbox checked={confirmed} onCheckedChange={(value) => (confirmed = value)} />I understand the shown effect will change this exact resource.</Label>{/if}
	{#if preview}<p class="rounded border border-sky-500/40 bg-sky-500/10 p-3 text-xs">Preview: {preview}</p>{/if}
	{#if result}<p class="rounded border border-emerald-500/40 bg-emerald-500/10 p-3 text-xs">{result}</p>{/if}
	{#if error}
		<p class="rounded border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive">Blocker: {errorBlocker}. {getErrorMessage(error)}</p>
	{/if}
</div>
