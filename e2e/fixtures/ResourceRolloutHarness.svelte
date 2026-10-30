<script lang="ts">
	import { setQueryClientContext, type QueryClient } from "@tanstack/svelte-query";
	import type { ComponentProps } from "svelte";
	import ResourceBrowser from "@/features/resources/ResourceBrowser.svelte";
	import { TooltipProvider } from "@/components/ui/svelte";
	const { queryClient, ...props }: ComponentProps<typeof ResourceBrowser> & { queryClient: QueryClient } = $props();
	// svelte-ignore state_referenced_locally -- Each mount owns a fixed client.
	setQueryClientContext(queryClient);
	let namespaceOverride = $state<string[]>();
	export function changeNamespace(namespace: string) { namespaceOverride = [namespace]; }
</script>

<div class="dark fixed inset-0 z-10 h-screen bg-background p-4 text-foreground" data-rollout-harness>
	<TooltipProvider><ResourceBrowser {...props} initialNamespaces={namespaceOverride ?? props.initialNamespaces} /></TooltipProvider>
</div>
