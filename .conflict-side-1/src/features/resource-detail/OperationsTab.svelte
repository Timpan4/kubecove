<script lang="ts">
	import { Button, Checkbox, Input, Label } from "@/components/ui/svelte";
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

	let { client, resource, kubeconfigSourceKey }: { client: TauriClient; resource: ResourceSummary; kubeconfigSourceKey?: string } = $props();
	let replicas = $state("1");
	let confirmed = $state(false);
	let preview = $state("");
	let previewFingerprint = $state("");
	let result = $state("");
	let error = $state("");
	let busy = $state(false);
	const target = $derived({ clusterContext: resource.cluster, namespace: resource.namespace, kind: resource.kind, name: resource.name });
	const canScale = $derived(resource.kind === "Deployment" || resource.kind === "StatefulSet");
	const canRestart = $derived(canScale || resource.kind === "DaemonSet");
	const canDelete = $derived(resource.kind === "Pod" || resource.kind === "ConfigMap");

	function clearFeedback() { preview = ""; previewFingerprint = ""; result = ""; error = ""; }
	function reset() { confirmed = false; clearFeedback(); }
	function fingerprint(action: "scale" | "restart" | "delete") {
		return JSON.stringify({ action, target, replicas: action === "scale" ? Number(replicas) : undefined });
	}
	async function run(action: "scale" | "restart" | "delete", execute: boolean) {
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
		result = ""; error = ""; busy = true;
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
		} catch (caught) { error = caught instanceof Error ? caught.message : String(caught); }
		finally { busy = false; }
	}
</script>

<div class="space-y-4 rounded-md border bg-muted/20 p-4 text-sm">
	<div><h3 class="font-medium">Guarded operations</h3><p class="mt-1 text-xs text-muted-foreground">Operations are limited to this exact selected resource.</p></div>
	<div class="rounded border bg-background p-3 font-mono text-xs">
		<div>Context: {target.clusterContext}</div><div>Namespace: {target.namespace ?? "cluster-scoped"}</div><div>Kind: {target.kind}</div><div>Name: {target.name}</div>
	</div>
	{#if canScale}
		<div class="space-y-2"><Label for="operation-replicas">Desired replicas</Label><Input id="operation-replicas" type="number" min="0" bind:value={replicas} oninput={reset} />
			<div class="flex gap-2"><Button variant="outline" disabled={busy} onclick={() => run("scale", false)}>Preview scale</Button><Button disabled={busy || !confirmed || !previewFingerprint} onclick={() => run("scale", true)}>Scale workload</Button></div></div>
	{/if}
	{#if canRestart}<div class="flex gap-2"><Button variant="outline" disabled={busy} onclick={() => run("restart", false)}>Preview restart</Button><Button disabled={busy || !confirmed || !previewFingerprint} onclick={() => run("restart", true)}>Rollout restart</Button></div>{/if}
	{#if canDelete}<div class="flex gap-2"><Button variant="outline" disabled={busy} onclick={() => run("delete", false)}>Preview delete</Button><Button variant="destructive" disabled={busy || !confirmed || !previewFingerprint} onclick={() => run("delete", true)}>Delete resource</Button></div>{/if}
	{#if !canScale && !canRestart && !canDelete}<p class="text-xs text-muted-foreground">No guarded operation is supported for this kind.</p>{/if}
	{#if canScale || canRestart || canDelete}<Label class="gap-2 rounded border bg-background p-3 text-xs text-muted-foreground"><Checkbox checked={confirmed} onCheckedChange={(value) => (confirmed = value)} />I understand the shown effect will change this exact resource.</Label>{/if}
	{#if preview}<p class="rounded border border-sky-500/40 bg-sky-500/10 p-3 text-xs">Preview: {preview}</p>{/if}
	{#if result}<p class="rounded border border-emerald-500/40 bg-emerald-500/10 p-3 text-xs">{result}</p>{/if}
	{#if error}<p class="rounded border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive">{error}</p>{/if}
</div>
