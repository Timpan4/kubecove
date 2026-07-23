<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import { AlertCircle, Link2, ShieldAlert } from "lucide-svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Button,
		Field,
		FieldGroup,
		FieldLabel,
		Input,
		Switch,
		Textarea,
	} from "@/components/ui/svelte";
	import { settingsStore } from "@/lib/settings-store";
	import {
		connectArgoServer,
		createTauriClient,
		disconnectArgoServer,
		discoverArgoServers,
		forgetArgoCredential,
	} from "@/lib/tauri";

	let { clusterContext, workspaceId, kubeconfigEnvVar }: {
		clusterContext?: string;
		workspaceId?: string;
		kubeconfigEnvVar?: string;
	} = $props();
	const client = createTauriClient();
	const settings = $derived($settingsStore);
	let url = $state("");
	let token = $state("");
	let username = $state("");
	let password = $state("");
	let customCa = $state("");
	let insecureTls = $state(false);
	let rememberCredential = $state(false);
	let loginMode = $state<"token" | "local">("token");
	let busy = $state(false);
	let error = $state<string | null>(null);
	let connected = $state<string | null>(null);
	const discovered = createQuery(() => ({
		queryKey: ["argo-server-discovery", clusterContext ?? "", kubeconfigEnvVar ?? ""],
		queryFn: () => discoverArgoServers(client, clusterContext!, kubeconfigEnvVar),
		enabled: Boolean(clusterContext),
		staleTime: 60_000,
	}));

	function profileId(serverUrl: string) {
		return `argo:${workspaceId ?? "global"}:${clusterContext ?? "global"}:${serverUrl.trim().toLowerCase()}`;
	}

	async function connect(saved?: (typeof settings.argoProfiles)[number]) {
		const serverUrl = saved?.url ?? url.trim();
		if (!serverUrl) return;
		busy = true;
		error = null;
		try {
			const id = saved?.id ?? profileId(serverUrl);
			const result = await connectArgoServer(client, {
				id,
				serverUrl,
				token: saved ? undefined : token || undefined,
				username: saved ? undefined : username || undefined,
				password: saved ? undefined : password || undefined,
					insecureTls: saved ? false : insecureTls,
				customCaPem: saved || !customCa ? undefined : [...new TextEncoder().encode(customCa)],
				rememberCredential: saved?.rememberCredential ?? rememberCredential,
				clusterContext,
				workspaceId,
			});
			if (result.profile) {
				const profile = result.profile;
				settings.setArgoProfiles([
					...settings.argoProfiles.filter((item) => item.id !== profile.id && item.id !== saved?.id),
					{ id: profile.id, url: profile.url, clusterContext: profile.clusterContext ?? undefined, workspaceId: profile.workspaceId ?? undefined, rememberCredential: profile.rememberCredential },
				]);
			}
			connected = result.profile?.id ?? id;
		} catch (caught) {
			error = caught instanceof Error ? caught.message : String(caught);
		} finally {
			// Credentials never persist in component state after submit.
			token = "";
			username = "";
			password = "";
			customCa = "";
			busy = false;
		}
	}
</script>

<FieldGroup>
	<Field>
		<FieldLabel>Argo CD server URL</FieldLabel>
		<Input bind:value={url} type="url" placeholder="https://argocd.example.com" />
	</Field>
	<div class="flex gap-2">
		<Button type="button" variant={loginMode === "token" ? "secondary" : "outline"} onclick={() => (loginMode = "token")}>Token</Button>
		<Button type="button" variant={loginMode === "local" ? "secondary" : "outline"} onclick={() => (loginMode = "local")}>Local login</Button>
	</div>
	{#if loginMode === "token"}
		<Field><FieldLabel>Token</FieldLabel><Input bind:value={token} type="password" autocomplete="off" /></Field>
	{:else}
		<Field><FieldLabel>Username</FieldLabel><Input bind:value={username} autocomplete="username" /></Field>
		<Field><FieldLabel>Password</FieldLabel><Input bind:value={password} type="password" autocomplete="current-password" /></Field>
	{/if}
	<Field><FieldLabel>Custom CA PEM (session only)</FieldLabel><Textarea bind:value={customCa} rows={3} autocomplete="off" /></Field>
	<Field class="flex-row items-center justify-between"><FieldLabel>Remember credential in native keyring</FieldLabel><Switch checked={rememberCredential} onCheckedChange={(value) => (rememberCredential = value)} /></Field>
	<Field class="flex-row items-center justify-between"><FieldLabel>Accept invalid TLS certificate for this session</FieldLabel><Switch checked={insecureTls} onCheckedChange={(value) => (insecureTls = value)} /></Field>
	{#if insecureTls}
		<Alert variant="destructive"><ShieldAlert /><AlertTitle>Insecure session</AlertTitle><AlertDescription>Certificate validation is disabled and never saved.</AlertDescription></Alert>
	{/if}
	{#if error}<Alert variant="destructive"><AlertCircle /><AlertTitle>Connection failed</AlertTitle><AlertDescription>{error}</AlertDescription></Alert>{/if}
	<Button type="button" disabled={busy || !url.trim()} onclick={() => connect()}><Link2 />{busy ? "Connecting…" : "Connect"}</Button>
</FieldGroup>

{#if settings.argoProfiles.length > 0}
		<div class="mt-4 flex flex-col gap-2"><p class="text-sm font-medium">Saved server profiles</p>{#each settings.argoProfiles.filter((profile) => !clusterContext || profile.clusterContext === clusterContext) as profile}
			<div class="flex items-center justify-between gap-2 rounded-md border p-2 text-sm"><span class="truncate">{profile.url}</span><div class="flex items-center gap-2"><Button size="sm" type="button" onclick={() => connect(profile)}>{connected === profile.id ? "Connected" : "Reconnect"}</Button><Button size="sm" variant="ghost" type="button" onclick={() => disconnectArgoServer(client, profile.id)}>Disconnect</Button><Button size="sm" variant="ghost" type="button" onclick={async () => { await forgetArgoCredential(client, { ...profile, transport: "connected" }); settings.setArgoProfiles(settings.argoProfiles.filter((item) => item.id !== profile.id)); }}>Forget</Button></div></div>
	{/each}</div>
{/if}

{#if discovered.data?.length}
	<div class="mt-4 flex flex-col gap-2"><p class="text-sm font-medium">Discovered servers</p>{#each discovered.data as server}
		<div class="rounded-md border p-2 text-sm"><span>{server.name}</span>{#if server.unavailableReason}<p class="mt-1 text-muted-foreground">{server.unavailableReason}</p>{/if}</div>
	{/each}</div>
{/if}
