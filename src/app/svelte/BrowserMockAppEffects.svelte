<script lang="ts">
	import { useQueryClient } from "@tanstack/svelte-query";
	import { onMount } from "svelte";
	import { desktopRemoteConnection } from "@/lib/remote-runtime";
	import { isAppUpdatesEnabled } from "@/lib/release-channel";
	import { scheduleAutomaticUpdateCheck } from "./appUpdateStore";

	const queryClient = useQueryClient();
	let remoteScopeInitialized = false;
	let lastRemoteScope: string | null = null;

	onMount(() => {
		if (!isAppUpdatesEnabled()) return;
		return scheduleAutomaticUpdateCheck();
	});

	$effect(() => {
		const remoteScope = $desktopRemoteConnection?.serverUrl ?? null;
		if (!remoteScopeInitialized) {
			remoteScopeInitialized = true;
			lastRemoteScope = remoteScope;
			return;
		}
		if (lastRemoteScope === remoteScope) return;
		lastRemoteScope = remoteScope;
		void queryClient.resetQueries();
	});

</script>
