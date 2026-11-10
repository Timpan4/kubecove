<script lang="ts">
	import { onMount } from "svelte";
	import { Button, Input, Label } from "@/components/ui/svelte";
	import RemoteSessionNotice from "@/features/remote-connection/RemoteSessionNotice.svelte";
	import { messageFromError } from "@/lib/error-redaction";
	import { isRemoteLive, remoteNotice, signInNotes } from "@/lib/remote-notice";
	import {
		connectDesktopRemote,
		desktopRemoteConnection,
		disconnectDesktopRemote,
		initializeDesktopRemote,
		signOutDesktopRemote,
	} from "@/lib/remote-runtime";

	let serverUrl = $state("");
	let token = $state("");
	let pending = $state(false);
	let error = $state<string | null>(null);
	const live = $derived($desktopRemoteConnection !== null && isRemoteLive($desktopRemoteConnection));
	const notice = $derived($desktopRemoteConnection ? remoteNotice($desktopRemoteConnection) : null);
	// A token server that is not connected takes the token again; a single sign-on server signs in instead.
	const needsToken = $derived(!$desktopRemoteConnection || ($desktopRemoteConnection.authMode === "token" && !live));

	$effect(() => {
		if ($desktopRemoteConnection) serverUrl = $desktopRemoteConnection.serverUrl;
	});

	onMount(() => {
		void initializeDesktopRemote().catch((reason) => {
			error = messageFromError(reason);
		});
	});

	async function run<Result>(action: () => Promise<Result>) {
		pending = true;
		error = null;
		try {
			await action();
			token = "";
		} catch (reason) {
			error = messageFromError(reason);
		} finally {
			pending = false;
		}
	}

	// An empty token selects the server by its sign-in mode (ADR 0031).
	const connect = () => run(() => connectDesktopRemote(serverUrl, token));
	const disconnect = () => run(disconnectDesktopRemote);
	const signOut = () => run(signOutDesktopRemote);
</script>

{#if $desktopRemoteConnection}
	<div class="mb-3 flex flex-col gap-3 text-sm">
		{#if live}
			<p>Connected to <span class="font-medium">{$desktopRemoteConnection.serverUrl}</span>.</p>
			<p class="text-muted-foreground">Kubernetes requests and port forwards run through this server. Forwarded ports listen only on 127.0.0.1 on this laptop.</p>
			<p class="text-muted-foreground">Server {$desktopRemoteConnection.serverVersion}. Session grace: {$desktopRemoteConnection.sessionGraceSeconds} seconds.</p>
			{#each signInNotes($desktopRemoteConnection) as note (note)}<p class="text-xs text-muted-foreground">{note}</p>{/each}
			<div class="flex gap-2">
				{#if $desktopRemoteConnection.authMode === "oidc"}
					<Button type="button" variant="outline" disabled={pending} onclick={signOut}>Sign out</Button>
				{/if}
				<Button type="button" variant="destructive" disabled={pending} onclick={disconnect}>{pending ? "Disconnecting…" : "Disconnect"}</Button>
			</div>
		{:else}
			<p class="text-muted-foreground">{$desktopRemoteConnection.serverUrl} is selected but disconnected. Cluster commands will not fall back to this laptop.</p>
			{#if notice}
				<RemoteSessionNotice {notice} notes={signInNotes($desktopRemoteConnection)} onError={(message) => (error = message)} />
			{/if}
		{/if}
	</div>
{/if}
{#if needsToken}
	<form class="flex max-w-xl flex-col gap-3" onsubmit={(event) => { event.preventDefault(); void connect(); }}>
		<div class="flex flex-col gap-2">
			<Label for="remote-server-url">Server URL</Label>
			<Input id="remote-server-url" type="url" autocomplete="url" placeholder="https://kube.example.com" bind:value={serverUrl} required />
		</div>
		<div class="flex flex-col gap-2">
			<Label for="remote-server-token">Server token</Label>
			<Input id="remote-server-token" type="password" autocomplete="off" bind:value={token} />
			<p class="text-xs text-muted-foreground">The token stays in native Rust memory until you disconnect or close the app.</p>
			<p class="text-xs text-muted-foreground">Leave the token empty to sign in to a single sign-on server through your browser.</p>
		</div>
		<div><Button type="submit" disabled={pending || !serverUrl.trim()}>{pending ? "Connecting…" : "Connect"}</Button></div>
	</form>
{/if}
{#if error}<p role="alert" class="mt-3 text-sm text-destructive">{error}</p>{/if}
