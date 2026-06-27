<script lang="ts">
	import { QueryClient, QueryClientProvider } from "@tanstack/svelte-query";
	import App from "./App.svelte";
	import { queryRetry } from "@/lib/query-retry";

	const queryClient = new QueryClient({
		defaultOptions: {
			queries: {
				staleTime: 30_000,
				retry: queryRetry,
			},
		},
	});
</script>

<QueryClientProvider client={queryClient}>
	<App />
</QueryClientProvider>
