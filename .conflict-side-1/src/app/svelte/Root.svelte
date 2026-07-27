<script lang="ts">
	import { QueryClient, QueryClientProvider } from "@tanstack/svelte-query";
	import App from "./App.svelte";
	import { configureLargeQueryRetention } from "@/lib/query-retention";
	import { queryRetry } from "@/lib/query-retry";

	const queryClient = new QueryClient({
		defaultOptions: {
			queries: {
				staleTime: 30_000,
				retry: queryRetry,
			},
		},
	});
	configureLargeQueryRetention(queryClient);
</script>

<QueryClientProvider client={queryClient}>
	<App />
</QueryClientProvider>
