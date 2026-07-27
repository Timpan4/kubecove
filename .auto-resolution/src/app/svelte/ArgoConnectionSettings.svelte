<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { AlertCircle, Link2, ShieldAlert } from "lucide-svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Button,
		Field,
		FieldDescription,
		FieldGroup,
		FieldLabel,
		Input,
		Switch,
		Textarea,
	} from "@/components/ui/svelte";
	import {
		argoEndpointIdentity,
		eligibleArgoProfiles,
		upsertArgoProfileInSavedOrder,
	} from "@/lib/argo-connection-policy";
	import { settingsStore } from "@/lib/settings-store";
	import {
		connectArgoServer,
		createTauriClient,
		disconnectArgoServer,
		discoverArgoServers,
		forgetArgoCredential,
		getArgoConnectionStatus,
	} from "@/lib/tauri";
	import type { ArgoServerEndpoint } from "@/lib/types";

	let { clusterContext, workspaceId, kubeconfigEnvVar }: {
		clusterContext?: string;
		workspaceId?: string;
		kubeconfigEnvVar?: string;
	} = $props();
	const client = createTauriClient();
	const queryClient = useQueryClient();
	const settings = $derived($settingsStore);
	let connectionMode = $state<"external" | "serviceTunnel">("external");
	let url = $state("");
	let selectedCapabilityId = $state("");
	let tunnelScheme = $state<"https" | "http">("https");
	let rootPath = $state("");
	let tlsServerName = $state("");
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
	const matchingProfiles = $derived(
		clusterContext && workspaceId
			? eligibleArgoProfiles(
					settings.argoProfiles,
					clusterContext,
					workspaceId,
					kubeconfigEnvVar ?? "",
				)
			: settings.argoProfiles,
	);
	const connectionStatuses = createQuery(() => ({
		queryKey: ["argo-connection-status", clusterContext ?? "", workspaceId ?? "", matchingProfiles.map((profile) => profile.id).join(",")],
		queryFn: () => Promise.all(matchingProfiles.map(async (profile) => [profile.id, await getArgoConnectionStatus(client, profile.id)] as const)),
		enabled: matchingProfiles.length > 0,
		staleTime: 5_000,
	}));
	const discovered = createQuery(() => ({
		queryKey: ["argo-server-discovery", clusterContext ?? "", kubeconfigEnvVar ?? ""],
		queryFn: () => discoverArgoServers(client, clusterContext!, kubeconfigEnvVar),
		enabled: Boolean(clusterContext),
		staleTime: 60_000,
	}));
	const tunnelCapabilities = $derived(
		discovered.data?.filter(
			(server) => server.endpoint?.kind === "serviceTunnel" && !server.unavailableReason,
		) ?? [],
	);
	const unavailableDiscoveries = $derived(
		discovered.data?.filter((server) => server.unavailableReason) ?? [],
	);
	const selectedCapability = $derived(
		tunnelCapabilities.find((server) => server.id === selectedCapabilityId),
	);
	const tunnelEndpoint = $derived(
		selectedCapability?.endpoint?.kind === "serviceTunnel"
			? {
					...selectedCapability.endpoint,
					scheme: tunnelScheme,
					...(rootPath.trim() ? { rootPath: rootPath.trim() } : {}),
					...(tlsServerName.trim() ? { tlsServerName: tlsServerName.trim() } : {}),
				}
			: null,
	);
	const draftEndpoint = $derived(
		connectionMode === "external"
			? url.trim()
				? { kind: "externalHttps" as const, url: url.trim() }
				: null
			: tunnelEndpoint,
	);

	$effect(() => {
		connected = connectionStatuses.data?.find(([, status]) => status.connected)?.[0] ?? null;
	});

	function endpointUrl(endpoint: ArgoServerEndpoint): string {
		return endpoint.kind === "externalHttps"
			? endpoint.url
			: `${endpoint.scheme}://${endpoint.serviceName}.${endpoint.namespace}.svc${endpoint.rootPath ?? ""}`;
	}

	function endpointLabel(endpoint: ArgoServerEndpoint): string {
		return endpoint.kind === "externalHttps"
			? endpoint.url
			: `Private Kubernetes service tunnel: ${endpoint.namespace}/${endpoint.serviceName} (${endpoint.scheme.toUpperCase()})`;
	}

	function profileId(endpoint: ArgoServerEndpoint) {
		return `argo:${workspaceId ?? "global"}:${clusterContext ?? "global"}:${kubeconfigEnvVar ?? "global"}:${argoEndpointIdentity(endpoint)}`;
	}

	async function connect(saved?: (typeof settings.argoProfiles)[number]) {
		const endpoint = saved?.endpoint ?? draftEndpoint;
		if (!endpoint) return;
		busy = true;
		error = null;
		try {
			const id = saved?.id ?? profileId(endpoint);
			const result = await connectArgoServer(client, {
				id,
				serverUrl: endpointUrl(endpoint),
				endpoint,
				token: saved ? undefined : token || undefined,
				username: saved ? undefined : username || undefined,
				password: saved ? undefined : password || undefined,
				insecureTls: saved ? false : insecureTls,
				customCaPem: saved || !customCa ? undefined : [...new TextEncoder().encode(customCa)],
				rememberCredential: saved?.rememberCredential ?? rememberCredential,
				clusterContext,
				kubeconfigEnvVar,
				workspaceId,
			});
			if (result.profile) {
				const profile = result.profile;
				settings.setArgoProfiles(
					upsertArgoProfileInSavedOrder(
						settings.argoProfiles,
						{
							id: profile.id,
							endpoint: profile.endpoint,
							clusterContext: profile.clusterContext ?? undefined,
							workspaceId: profile.workspaceId ?? undefined,
							kubeconfigSourceKey: profile.kubeconfigSourceKey,
							rememberCredential: profile.rememberCredential,
						},
						saved?.id,
					),
				);
			}
			connected = result.profile?.id ?? id;
			void queryClient.invalidateQueries({ queryKey: ["argo-connection-status"] });
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

	async function disconnect(id: string) {
		error = null;
		try {
			await disconnectArgoServer(client, id);
			if (connected === id) connected = null;
			void queryClient.invalidateQueries({ queryKey: ["argo-connection-status"] });
		} catch (caught) {
			error = caught instanceof Error ? caught.message : String(caught);
		}
	}

	async function forget(profile: (typeof settings.argoProfiles)[number]) {
		error = null;
		try {
			await forgetArgoCredential(client, {
				...profile,
				url: endpointUrl(profile.endpoint),
				transport: "connected",
			});
			settings.setArgoProfiles(settings.argoProfiles.filter((item) => item.id !== profile.id));
			if (connected === profile.id) connected = null;
			void queryClient.invalidateQueries({ queryKey: ["argo-connection-status"] });
		} catch (caught) {
			error = caught instanceof Error ? caught.message : String(caught);
		}
	}
</script>

{#if clusterContext}
	<FieldGroup>
		<Field>
			<FieldLabel>Connection type</FieldLabel>
			<div class="flex gap-2">
				<Button type="button" variant={connectionMode === "external" ? "secondary" : "outline"} aria-pressed={connectionMode === "external"} onclick={() => (connectionMode = "external")}>External HTTPS</Button>
				<Button type="button" variant={connectionMode === "serviceTunnel" ? "secondary" : "outline"} aria-pressed={connectionMode === "serviceTunnel"} onclick={() => (connectionMode = "serviceTunnel")}>Private Kubernetes service tunnel</Button>
			</div>
		</Field>
		{#if connectionMode === "external"}
			<Field>
				<FieldLabel>Argo CD server URL</FieldLabel>
				<Input bind:value={url} type="url" placeholder="https://argocd.example.com" />
			</Field>
		{:else}
			<Alert>
				<AlertTitle>Private Kubernetes service tunnel</AlertTitle>
				<AlertDescription>Requires a readable selector-backed Argo CD Service, a TCP port, and a ready target Pod. The tunnel is private to this connection.</AlertDescription>
			</Alert>
			{#if discovered.isPending}
				<p class="text-sm text-muted-foreground">Discovering eligible Argo CD Services…</p>
			{:else if tunnelCapabilities.length > 0}
				<Field>
					<FieldLabel for="argo-service-port">Discovered Service port</FieldLabel>
					<select id="argo-service-port" bind:value={selectedCapabilityId} class="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm">
						<option value="">Select a Service port</option>
						{#each tunnelCapabilities as server}
							<option value={server.id}>{server.namespace}/{server.name} — port {server.endpoint?.kind === "serviceTunnel" ? server.endpoint.servicePort : ""}</option>
						{/each}
					</select>
					<FieldDescription>Only discovered selector-backed TCP Services can be connected.</FieldDescription>
				</Field>
				<Field>
					<FieldLabel>Tunnel protocol</FieldLabel>
					<div class="flex gap-2"><Button type="button" variant={tunnelScheme === "https" ? "secondary" : "outline"} aria-pressed={tunnelScheme === "https"} onclick={() => (tunnelScheme = "https")}>HTTPS</Button><Button type="button" variant={tunnelScheme === "http" ? "secondary" : "outline"} aria-pressed={tunnelScheme === "http"} onclick={() => (tunnelScheme = "http")}>HTTP</Button></div>
				</Field>
				<Field><FieldLabel for="argo-root-path">Root path (optional)</FieldLabel><Input id="argo-root-path" bind:value={rootPath} placeholder="/argo-cd" /></Field>
				<Field><FieldLabel for="argo-tls-server-name">TLS server name (optional)</FieldLabel><Input id="argo-tls-server-name" bind:value={tlsServerName} placeholder="argocd.internal" /></Field>
			{:else}
				<p class="text-sm text-muted-foreground">No eligible Argo CD Service tunnel was discovered for this cluster.</p>
			{/if}
		{/if}
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
		<Field orientation="horizontal"><FieldLabel>Remember credential in native keyring</FieldLabel><Switch checked={rememberCredential} onCheckedChange={(value) => (rememberCredential = value)} /></Field>
		<Field orientation="horizontal"><FieldLabel>Accept invalid TLS certificate for this session</FieldLabel><Switch checked={insecureTls} onCheckedChange={(value) => (insecureTls = value)} /></Field>
		{#if insecureTls}
			<Alert variant="destructive"><ShieldAlert /><AlertTitle>Insecure session</AlertTitle><AlertDescription>Certificate validation is disabled and never saved.</AlertDescription></Alert>
		{/if}
		{#if error}<Alert variant="destructive"><AlertCircle /><AlertTitle>Connection failed</AlertTitle><AlertDescription>{error}</AlertDescription></Alert>{/if}
		<Button type="button" disabled={busy || !draftEndpoint} onclick={() => connect()}><Link2 />{busy ? "Connecting…" : "Connect"}</Button>
	</FieldGroup>
{:else}
	<p class="text-sm text-muted-foreground">Open Settings from a workspace to discover or connect an Argo CD server.</p>
{/if}

{#if matchingProfiles.length > 0}
	<div class="mt-4 flex flex-col gap-2"><p class="text-sm font-medium">Saved server profiles</p>{#each matchingProfiles as profile}
		<div class="flex items-center justify-between gap-2 rounded-md border p-2 text-sm"><span class="truncate">{endpointLabel(profile.endpoint)}</span><div class="flex items-center gap-2">{#if clusterContext}<Button size="sm" type="button" onclick={() => connect(profile)}>{connected === profile.id ? "Connected" : "Reconnect"}</Button>{/if}<Button size="sm" variant="ghost" type="button" onclick={() => disconnect(profile.id)}>Disconnect</Button><Button size="sm" variant="ghost" type="button" onclick={() => forget(profile)}>Forget</Button></div></div>
	{/each}</div>
{/if}

{#if unavailableDiscoveries.length > 0}
	<div class="mt-4 flex flex-col gap-2"><p class="text-sm font-medium">Unavailable discovered services</p>{#each unavailableDiscoveries as server}
		<div class="rounded-md border p-2 text-sm"><span>{server.name}</span><p class="mt-1 text-muted-foreground">{server.unavailableReason}</p></div>
	{/each}</div>
{/if}
