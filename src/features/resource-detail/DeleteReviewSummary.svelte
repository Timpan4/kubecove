<script lang="ts">
	import { Badge } from "@/components/ui/svelte";
	import type { ResourceActionPreview } from "@/lib/types";

	// The exact delete target and what follows it, shown above the folded
	// dry-run diff so the user can confirm the target without typing its name.
	let { preview }: { preview: ResourceActionPreview } = $props();

	const target = $derived(preview.target);
	const controller = $derived(preview.controller);
	const consequence = $derived(
		controller?.recreates
			? `${controller.kind} ${controller.name} normally creates a replacement ${target.kind}.`
			: controller
				? `${controller.kind} ${controller.name} controls this ${target.kind}, but KubeCove cannot confirm it will create a replacement.`
				: "No controlling owner was found. The delete is permanent unless other automation, such as GitOps, recreates it.",
	);
</script>

<section class="mt-4 rounded-md border bg-card px-4 py-3" aria-label="Delete target">
	<div class="flex flex-wrap items-center gap-1.5">
		<Badge variant="secondary">{target.kind}</Badge>
		<Badge variant="outline" class="text-amber-300">{target.clusterContext}</Badge>
	</div>
	<p class="mt-2 break-all font-mono text-sm font-semibold">{target.name}</p>
	<dl class="mt-2 grid grid-cols-[7rem_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
		<dt class="text-muted-foreground">Context</dt><dd>{target.clusterContext}</dd>
		{#if target.namespace}<dt class="text-muted-foreground">Namespace</dt><dd class="font-mono">{target.namespace}</dd>{/if}
		{#if controller}<dt class="text-muted-foreground">Controlled by</dt><dd class="break-all font-mono">{controller.kind} {controller.name}</dd>{/if}
	</dl>
</section>
<section class="mt-3 rounded-md border bg-muted/15" aria-label="What happens">
	<h3 class="border-b px-3 py-2 text-xs font-medium">What happens</h3>
	<ul class="grid gap-1.5 px-3 py-2 text-xs">
		<li class="flex items-start gap-2"><span class="mt-1.5 size-1.5 shrink-0 rounded-full bg-destructive" aria-hidden="true"></span>The {target.kind} is removed from the cluster.</li>
		<li class="flex items-start gap-2" data-delete-consequence><span class={`mt-1.5 size-1.5 shrink-0 rounded-full ${controller?.recreates ? "bg-emerald-400" : "bg-amber-400"}`} aria-hidden="true"></span>{consequence}</li>
	</ul>
</section>
