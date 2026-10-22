<script lang="ts">
	import { onMount } from "svelte";
	import { QueryClient, QueryClientProvider } from "@tanstack/svelte-query";
	import App from "./App.svelte";
	import BrowserSessionGate from "./BrowserSessionGate.svelte";
	import { Button } from "@/components/ui/svelte";
	import { getBrowserSessionState, initializeBrowserSession, signOutBrowserSession, subscribeBrowserSession, type BrowserSessionState } from "@/lib/browser-runtime";
	import { isBrowserRuntime } from "@/lib/tauri-runtime";
	import { configureQueryCatalog } from "@/lib/queryKeys";
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
	const browserRuntime = isBrowserRuntime();
	let browserSession = $state<BrowserSessionState>(getBrowserSessionState());
	let signingOut = $state(false);
	let signOutError = $state<string | null>(null);
	async function signOut() {
		signingOut = true;
		signOutError = null;
		try {
			await signOutBrowserSession();
		} catch (error) {
			signOutError = error instanceof Error ? error.message : String(error);
		} finally {
			signingOut = false;
		}
	}
	onMount(() => {
		if (!browserRuntime) return;
		const unsubscribe = subscribeBrowserSession((state) => {
			browserSession = state;
		});
		void initializeBrowserSession();
		return unsubscribe;
	});
	configureQueryCatalog(queryClient);
	attachMemoryProfile(queryClient);
</script>

<QueryClientProvider client={queryClient}>
	{#if !browserRuntime}
		<App />
	{:else if browserSession.status === "ready"}
		<div class="flex h-dvh min-h-0 flex-col overflow-hidden">
			<div class="shrink-0 border-b bg-muted/30 px-4 py-2 text-xs text-muted-foreground">
				<div class="flex items-center justify-between gap-3">
					<span>Browser session on {window.location.host}. Port-forward listeners bind to the server loopback.</span>
					{#if browserSession.authenticationMode === "oidc"}
						<Button type="button" variant="outline" size="sm" disabled={signingOut} onclick={signOut}>
							{signingOut ? "Signing out…" : "Sign out"}
						</Button>
					{/if}
				</div>
				{#if signOutError}
					<p class="mt-1 text-destructive">{signOutError}</p>
				{/if}
			</div>
			{#if browserSession.error || browserSession.channelGap > 0}
				<div class="shrink-0 border-b bg-warning-surface px-4 py-2 text-xs text-warning-foreground">
					{browserSession.error ?? ""}
					{#if browserSession.channelGap > 0}
						{browserSession.channelGap} live messages were dropped. Refresh affected views.
					{/if}
				</div>
			{/if}
			<div class="min-h-0 flex-1 overflow-y-auto">
				<App />
			</div>
		</div>
	{:else}
		<BrowserSessionGate session={browserSession} />
	{/if}
</QueryClientProvider>
