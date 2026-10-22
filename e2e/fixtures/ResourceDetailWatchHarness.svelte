<script lang="ts">
	import { setQueryClientContext, type QueryClient } from "@tanstack/svelte-query";
	import type { ComponentProps } from "svelte";
	import ResourceDetailPanel from "@/features/resource-detail/ResourceDetailPanel.svelte";

	const { queryClient, ..._props }: ComponentProps<typeof ResourceDetailPanel> & {
		queryClient: QueryClient;
	} = $props();
	// svelte-ignore state_referenced_locally -- Each regression mount owns one fixed client.
	setQueryClientContext(queryClient);
</script>

<ResourceDetailPanel {..._props} />
