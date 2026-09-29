<script lang="ts">
	import { QueryClient, QueryClientProvider } from "@tanstack/svelte-query";
	import App from "./App.svelte";
	import { configureFiniteReadQueryDefaults } from "@/lib/finite-read-lifecycle";
	import { configureLargeQueryRetention } from "@/lib/query-retention";
	import { queryRetry } from "@/lib/query-retry";
	import { attachMemoryProfile } from "@/lib/memory-profile";

	const queryClient = new QueryClient({
		defaultOptions: {
			queries: {
				staleTime: 30_000,
				retry: queryRetry,
			},
		},
	});
	configureFiniteReadQueryDefaults(queryClient);
	configureLargeQueryRetention(queryClient);
	attachMemoryProfile(queryClient);
</script>

<QueryClientProvider client={queryClient}>
	<App />
</QueryClientProvider>
