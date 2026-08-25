<script lang="ts">
	import { cn } from "@/lib/utils";

	let {
		context,
		namespace,
		kind,
		resource,
		operationScope,
		transport,
		description,
		class: className,
	}: {
		context: string;
		namespace?: string | null;
		kind: string;
		resource: string;
		operationScope: string;
		transport?: string;
		description?: string;
		class?: string;
	} = $props();
</script>

<div
	data-slot="operation-scope-card"
	class={cn("grid gap-1 rounded-md border bg-background/50 p-3 font-mono text-xs sm:grid-cols-2", className)}
>
	<div>Operation scope: {operationScope}</div>
	{#if transport}<div>Transport: {transport}</div>{/if}
	<div>Context: {context}</div>
	<div>Namespace: {namespace ?? "cluster-scoped"}</div>
	<div>Kind: {kind}</div>
	<div>Resource: {resource}</div>
	{#if description}
		<p class="font-sans text-muted-foreground sm:col-span-2">{description}</p>
	{/if}
</div>
