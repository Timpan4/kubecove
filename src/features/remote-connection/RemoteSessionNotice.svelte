<script lang="ts">
	import { copyText } from "@/components/copy-text";
	import { copyToast } from "@/components/CopyToast.svelte";
	import { Button } from "@/components/ui/svelte";
	import { messageFromError } from "@/lib/error-redaction";
	import { type RemoteNotice, type RemoteNoticeAction, REMOTE_NOTICE_LABELS } from "@/lib/remote-notice";
	import {
		cancelDesktopRemoteSignIn,
		desktopRemoteSignInLink,
		disconnectDesktopRemote,
		signInDesktopRemote,
		startNewDesktopRemoteSession,
	} from "@/lib/remote-runtime";

	let {
		notice,
		notes,
		onError,
	}: { notice: RemoteNotice; notes: readonly string[]; onError: (message: string | null) => void } = $props();
	let busy = $state(false);

	async function run(action: RemoteNoticeAction) {
		onError(null);
		if (action === "copySignInLink") {
			const link = await desktopRemoteSignInLink().catch(() => null);
			if (link) copyToast.set(await copyText(navigator.clipboard, link, "sign-in link"));
			return;
		}
		// The sign-in call lasts until the browser finishes or the user cancels, so its notice stays usable.
		const blocking = action !== "signIn";
		busy = blocking;
		try {
			if (action === "signIn") await signInDesktopRemote();
			else if (action === "cancelSignIn") await cancelDesktopRemoteSignIn();
			else if (action === "startNewSession") await startNewDesktopRemoteSession();
			else await disconnectDesktopRemote();
		} catch (reason) {
			onError(messageFromError(reason));
		} finally {
			if (blocking) busy = false;
		}
	}
</script>

<div class="flex flex-col gap-2 text-sm">
	{#if notice.title}<p class="font-medium">{notice.title}</p>{/if}
	<p>{notice.message}</p>
	{#each notes as note (note)}<p class="text-xs text-muted-foreground">{note}</p>{/each}
	<div class="flex flex-wrap gap-2">
		{#each notice.actions as action, index (action)}
			<Button
				type="button"
				variant={index === 0 ? "default" : "outline"}
				disabled={busy && action !== "cancelSignIn"}
				onclick={() => void run(action)}
			>
				{REMOTE_NOTICE_LABELS[action]}
			</Button>
		{/each}
	</div>
</div>
