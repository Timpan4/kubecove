<script lang="ts">
	import { remoteNotice, signInNotes } from "@/lib/remote-notice";
	import { desktopRemoteConnection } from "@/lib/remote-runtime";
	import RemoteSessionNotice from "./RemoteSessionNotice.svelte";

	const notice = $derived($desktopRemoteConnection ? remoteNotice($desktopRemoteConnection) : null);
	let error = $state<string | null>(null);
</script>

<!-- Shown in place of cluster views while the selected remote is not connected (ADR 0031). -->
{#if notice && $desktopRemoteConnection}
	<section class="flex h-full w-full items-center justify-center p-6">
		<div role="region" aria-label="Remote server notice" class="flex w-full max-w-md flex-col gap-3 rounded-lg border p-4">
			<RemoteSessionNotice {notice} notes={signInNotes($desktopRemoteConnection)} onError={(message) => (error = message)} />
			{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
		</div>
	</section>
{/if}
