<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { AlertCircle, Link2, RefreshCw, ShieldAlert } from "lucide-svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Badge,
		Button,
		Dialog,
		DialogContent,
		DialogDescription,
		DialogHeader,
		DialogTitle,
		Field,
		FieldDescription,
		FieldGroup,
		FieldLabel,
		Input,
		SegmentedControl,
		Switch,
		Textarea,
	} from "@/components/ui/svelte";
	import { eligibleArgoProfiles, upsertArgoProfileInSavedOrder } from "@/lib/argo-connection-policy";
	import { messageFromError } from "@/lib/error-redaction";
	import { argoConnectionStatusQueryOptions } from "@/lib/argo-application-inspection";
	import { finiteRead } from "@/lib/finite-read-lifecycle";
	import { queryKeys, queryMatchers } from "@/lib/queryKeys";
	import type { SavedArgoProfile } from "@/lib/settings";
	import { settingsStore } from "@/lib/settings-store";
	import {
		connectArgoServer,
		createTauriClient,
		disconnectArgoServer,
		discoverArgoServers,
		forgetArgoCredential,
		previewArgoTunnelTarget,
	} from "@/lib/tauri";
	import type { ArgoConfirmedTarget, ArgoServerEndpoint, ArgoTunnelTargetPreview } from "@/lib/types";
	import {
		argoEndpointLabel,
		argoEndpointUrl,
		argoProfileId,
		connectableArgoServers,
		formatCertificateSha256,
		reconnectRememberedArgoProfile,
		savedArgoProfile,
		type ArgoWorkspaceScope,
	} from "./argo-connection";

	let {
		open = $bindable(false),
		scope,
		workspaceName,
	}: { open?: boolean; scope: ArgoWorkspaceScope; workspaceName?: string } = $props();

	const client = createTauriClient();
	const queryClient = useQueryClient();
	const settings = $derived($settingsStore);
	const profiles = $derived(
		eligibleArgoProfiles(settings.argoProfiles, scope.clusterContext, scope.workspaceId, scope.kubeconfigSourceKey ?? ""),
	);
	const statuses = createQuery(() =>
		argoConnectionStatusQueryOptions(client, {
			profiles,
			queryKey: queryKeys.argoConnectionStatuses(scope.clusterContext, scope.workspaceId, profiles.map((profile) => profile.id), scope.kubeconfigSourceKey),
			enabled: open && profiles.length > 0,
		}),
	);
	const statusById = $derived(new Map(statuses.data ?? []));
	const discoveryKey = $derived(queryKeys.argoServerDiscovery(scope.clusterContext, scope.kubeconfigSourceKey));
	const discovery = createQuery(() => ({
		...finiteRead({
			queryKey: discoveryKey,
			label: "argo.discovery",
			read: (cancellable) => discoverArgoServers(client, scope.clusterContext, scope.kubeconfigSourceKey, cancellable),
		}),
		enabled: open,
		staleTime: 60_000,
	}));
	const servers = $derived(connectableArgoServers(discovery.data));
	const unavailableServers = $derived(discovery.data?.filter((server) => server.unavailableReason) ?? []);

	let sourceChoice = $state<"discovered" | "external" | null>(null);
	const source = $derived(sourceChoice ?? (servers.length > 0 ? "discovered" : "external"));
	let serverChoice = $state<string | null>(null);
	const selectedServer = $derived(servers.find((server) => server.id === serverChoice) ?? servers[0]);
	let schemeChoice = $state<"https" | "http" | null>(null);
	let rootPath = $state("");
	let tlsServerName = $state("");
	let url = $state("");
	let customCa = $state("");
	let insecureTls = $state(false);
	let loginMode = $state<"token" | "local">("token");
	let token = $state("");
	let username = $state("");
	let password = $state("");
	let rememberCredential = $state(false);
	let busy = $state<string | null>(null);
	let error = $state<string | null>(null);
	let reconfirm = $state<SavedArgoProfile | null>(null);
	let pending = $state<{ endpoint: ArgoServerEndpoint; saved?: SavedArgoProfile; preview: ArgoTunnelTargetPreview; argoLabeled: boolean } | null>(null);

	function draftEndpoint(): ArgoServerEndpoint | null {
		if (source === "external") return url.trim() ? { kind: "externalHttps", url: url.trim() } : null;
		const discovered = selectedServer?.endpoint;
		if (discovered?.kind !== "serviceTunnel") return null;
		return {
			...discovered,
			scheme: schemeChoice ?? discovered.scheme,
			rootPath: rootPath.trim() || discovered.rootPath,
			tlsServerName: tlsServerName.trim() || discovered.tlsServerName,
		};
	}
	const endpoint = $derived(draftEndpoint());
	const hasCredential = $derived(loginMode === "token" ? Boolean(token) : Boolean(username && password));

	function selectServer(id: string) {
		serverChoice = id;
		schemeChoice = null;
		rootPath = "";
		tlsServerName = "";
	}

	function isArgoLabeled(target: ArgoServerEndpoint) {
		return servers.some(
			(server) =>
				server.endpoint?.kind === "serviceTunnel" &&
				target.kind === "serviceTunnel" &&
				server.namespace === target.namespace &&
				server.name === target.serviceName &&
				server.argoLabeled,
		);
	}

	function refreshStatuses() {
		void queryClient.invalidateQueries({ predicate: queryMatchers.argoConnectionStatuses() });
	}

	// Credentials cross a private tunnel only after the user confirms its Pod and certificate.
	async function startConnect(target: ArgoServerEndpoint, saved?: SavedArgoProfile) {
		error = null;
		reconfirm = null;
		if (target.kind === "externalHttps") return connect(target, saved);
		busy = "Checking the tunnel target…";
		try {
			const preview = await previewArgoTunnelTarget(client, scope.clusterContext, target, scope.kubeconfigSourceKey);
			pending = { endpoint: target, saved, preview, argoLabeled: isArgoLabeled(target) };
		} catch (caught) {
			error = messageFromError(caught);
			void queryClient.invalidateQueries({ queryKey: discoveryKey });
		} finally {
			busy = null;
		}
	}

	async function connect(target: ArgoServerEndpoint, saved?: SavedArgoProfile, confirmedTarget?: ArgoConfirmedTarget) {
		pending = null;
		busy = "Connecting…";
		error = null;
		const external = target.kind === "externalHttps";
		try {
			const result = await connectArgoServer(client, {
				id: saved?.id ?? argoProfileId(scope, target),
				serverUrl: argoEndpointUrl(target),
				endpoint: target,
				token: loginMode === "token" ? token || undefined : undefined,
				username: loginMode === "local" ? username || undefined : undefined,
				password: loginMode === "local" ? password || undefined : undefined,
				insecureTls: external && insecureTls,
				customCaPem: external && customCa ? [...new TextEncoder().encode(customCa)] : undefined,
				rememberCredential: Boolean(saved?.rememberCredential) || rememberCredential,
				clusterContext: scope.clusterContext,
				kubeconfigEnvVar: scope.kubeconfigSourceKey,
				workspaceId: scope.workspaceId,
				confirmedTarget,
			});
			const profile = savedArgoProfile(result);
			if (profile) settings.setArgoProfiles(upsertArgoProfileInSavedOrder(settings.argoProfiles, profile, saved?.id));
			refreshStatuses();
		} catch (caught) {
			error = messageFromError(caught);
			// A rejected tunnel target is usually a replaced Pod; refetch so the next preview is current.
			if (confirmedTarget) void queryClient.invalidateQueries({ queryKey: discoveryKey });
		} finally {
			// Credentials never persist in component state after submit.
			token = "";
			username = "";
			password = "";
			customCa = "";
			busy = null;
		}
	}

	async function reconnect(profile: SavedArgoProfile) {
		error = null;
		reconfirm = null;
		if (!profile.rememberCredential) {
			if (!hasCredential) {
				error = "This profile does not remember its credential. Enter a token or local login below, then reconnect.";
				return;
			}
			return startConnect(profile.endpoint, profile);
		}
		busy = "Reconnecting…";
		try {
			await reconnectRememberedArgoProfile(client, scope, profile);
			refreshStatuses();
		} catch (caught) {
			error = messageFromError(caught);
			if (profile.endpoint.kind === "serviceTunnel") reconfirm = profile;
		} finally {
			busy = null;
		}
	}

	async function disconnect(profile: SavedArgoProfile) {
		error = null;
		try {
			await disconnectArgoServer(client, profile.id);
			refreshStatuses();
		} catch (caught) {
			error = messageFromError(caught);
		}
	}

	async function forget(profile: SavedArgoProfile) {
		error = null;
		try {
			await disconnectArgoServer(client, profile.id);
			await forgetArgoCredential(client, { ...profile, url: argoEndpointUrl(profile.endpoint), transport: "connected" });
			settings.setArgoProfiles(settings.argoProfiles.filter((item) => item.id !== profile.id));
			refreshStatuses();
		} catch (caught) {
			error = messageFromError(caught);
		}
	}
</script>

<Dialog {open} onOpenChange={(value: boolean) => (open = value)}>
	<DialogContent class="max-h-[90vh] max-w-2xl overflow-y-auto">
		<DialogHeader>
			<DialogTitle>Argo CD connection</DialogTitle>
			<DialogDescription>
				Saved for {workspaceName ? `workspace ${workspaceName}` : "this workspace"} on {scope.clusterContext}. Other workspaces keep their own connections.
			</DialogDescription>
		</DialogHeader>

		{#if profiles.length > 0}
			<section class="flex flex-col gap-2" aria-label="Saved Argo CD connections">
				{#each profiles as profile (profile.id)}
					{@const status = statusById.get(profile.id)}
					<div class="flex flex-wrap items-center justify-between gap-2 rounded-md border p-2 text-sm">
						<div class="min-w-0">
							<p class="truncate font-mono text-xs">{argoEndpointLabel(profile.endpoint)}</p>
							<p class="text-xs text-muted-foreground">
								{status?.connected ? `Connected${status.username ? ` as ${status.username}` : ""}` : "Not connected"}{profile.rememberCredential ? " · reconnects automatically" : ""}
							</p>
						</div>
						<div class="flex items-center gap-1">
							{#if status?.connected}
								<Button size="sm" variant="ghost" type="button" onclick={() => disconnect(profile)}>Disconnect</Button>
							{:else}
								<Button size="sm" type="button" disabled={busy !== null} onclick={() => reconnect(profile)}>Reconnect</Button>
							{/if}
							<Button size="sm" variant="ghost" type="button" onclick={() => forget(profile)}>Forget</Button>
						</div>
					</div>
				{/each}
			</section>
		{/if}

		<FieldGroup>
			<Field>
				<FieldLabel>Server</FieldLabel>
				<SegmentedControl
					value={source}
					options={[
						{ value: "discovered", label: "Found in cluster" },
						{ value: "external", label: "External URL" },
					]}
					onChange={(value: "discovered" | "external") => (sourceChoice = value)}
					ariaLabel="Argo CD server source"
				/>
			</Field>

			{#if source === "discovered"}
				{#if discovery.isPending}
					<p class="text-sm text-muted-foreground">Looking for Argo CD in this cluster…</p>
				{:else if discovery.isError}
					<Alert variant="destructive">
						<AlertCircle />
						<AlertTitle>Discovery failed</AlertTitle>
						<AlertDescription>
							<p>{messageFromError(discovery.error)}</p>
							<Button class="mt-2" size="sm" variant="outline" type="button" onclick={() => discovery.refetch()}><RefreshCw />Retry</Button>
						</AlertDescription>
					</Alert>
				{:else if servers.length === 0}
					<p class="text-sm text-muted-foreground">
						No Argo CD API server Service was found in the namespaces you can read. Core (headless) installs run without the API server, so there is nothing to connect to. The Kubernetes transport still lists Applications and runs refresh and sync. If the server runs elsewhere, use an external URL.
					</p>
				{:else}
					<div class="flex flex-col gap-1" role="radiogroup" aria-label="Discovered Argo CD servers">
						{#each servers as server (server.id)}
							{@const tunnel = server.endpoint?.kind === "serviceTunnel" ? server.endpoint : null}
							<button
								type="button"
								role="radio"
								aria-checked={server.id === selectedServer?.id}
								class="flex items-center justify-between gap-2 rounded-md border px-3 py-2 text-left text-sm aria-checked:border-primary aria-checked:bg-surface-1"
								onclick={() => selectServer(server.id)}
							>
								<span class="truncate font-mono text-xs">{server.namespace}/{server.name}:{tunnel?.servicePort} · {tunnel?.scheme.toUpperCase()}{tunnel?.rootPath ?? ""}</span>
								{#if !server.argoLabeled}<Badge variant="outline">No Argo CD label</Badge>{/if}
							</button>
						{/each}
					</div>
					<details class="text-sm">
						<summary class="cursor-pointer text-muted-foreground">Advanced</summary>
						<div class="mt-3 flex flex-col gap-3">
							<Field>
								<FieldLabel>Protocol</FieldLabel>
								<SegmentedControl
									value={schemeChoice ?? (selectedServer?.endpoint?.kind === "serviceTunnel" ? (selectedServer.endpoint.scheme as "https" | "http") : "https")}
									options={[
										{ value: "https", label: "HTTPS" },
										{ value: "http", label: "HTTP" },
									]}
									onChange={(value: "https" | "http") => (schemeChoice = value)}
									ariaLabel="Tunnel protocol"
								/>
								<FieldDescription>Detected from argocd-cmd-params-cm. HTTPS pins the server certificate you confirm.</FieldDescription>
							</Field>
							<Field><FieldLabel for="argo-root-path">Root path</FieldLabel><Input id="argo-root-path" bind:value={rootPath} placeholder={selectedServer?.endpoint?.kind === "serviceTunnel" ? (selectedServer.endpoint.rootPath ?? "/") : "/"} /></Field>
							<Field><FieldLabel for="argo-tls-server-name">TLS server name</FieldLabel><Input id="argo-tls-server-name" bind:value={tlsServerName} placeholder="<service>.<namespace>.svc" /></Field>
						</div>
					</details>
				{/if}
				{#if unavailableServers.length > 0}
					<details class="text-sm">
						<summary class="cursor-pointer text-muted-foreground">{unavailableServers.length} unavailable Argo CD Service{unavailableServers.length === 1 ? "" : "s"}</summary>
						<ul class="mt-2 flex flex-col gap-1">
							{#each unavailableServers as server (server.id)}
								<li class="text-xs"><span class="font-mono">{server.namespace}/{server.name}</span>: {server.unavailableReason}</li>
							{/each}
						</ul>
					</details>
				{/if}
			{:else}
				<Field>
					<FieldLabel for="argo-url">Argo CD server URL</FieldLabel>
					<Input id="argo-url" bind:value={url} type="url" placeholder="https://argocd.example.com" />
				</Field>
				<details class="text-sm">
					<summary class="cursor-pointer text-muted-foreground">Advanced</summary>
					<div class="mt-3 flex flex-col gap-3">
						<Field><FieldLabel for="argo-custom-ca">Custom CA PEM (session only)</FieldLabel><Textarea id="argo-custom-ca" bind:value={customCa} rows={3} autocomplete="off" /></Field>
						<Field orientation="horizontal"><FieldLabel>Accept invalid TLS certificate for this session</FieldLabel><Switch checked={insecureTls} onCheckedChange={(value) => (insecureTls = value)} /></Field>
						{#if insecureTls}
							<Alert variant="destructive"><ShieldAlert /><AlertTitle>Insecure session</AlertTitle><AlertDescription>Certificate validation is disabled and never saved.</AlertDescription></Alert>
						{/if}
					</div>
				</details>
			{/if}

			<Field>
				<FieldLabel>Sign in</FieldLabel>
				<SegmentedControl
					value={loginMode}
					options={[
						{ value: "token", label: "Token" },
						{ value: "local", label: "Local login" },
					]}
					onChange={(value: "token" | "local") => (loginMode = value)}
					ariaLabel="Argo CD sign-in method"
				/>
			</Field>
			{#if loginMode === "token"}
				<Field><FieldLabel for="argo-token">Token</FieldLabel><Input id="argo-token" bind:value={token} type="password" autocomplete="off" /></Field>
			{:else}
				<Field><FieldLabel for="argo-username">Username</FieldLabel><Input id="argo-username" bind:value={username} autocomplete="username" /></Field>
				<Field><FieldLabel for="argo-password">Password</FieldLabel><Input id="argo-password" bind:value={password} type="password" autocomplete="current-password" /></Field>
			{/if}
			<Field orientation="horizontal">
				<div>
					<FieldLabel>Remember credential in native keyring</FieldLabel>
					<FieldDescription>Reconnects automatically when this workspace opens.</FieldDescription>
				</div>
				<Switch checked={rememberCredential} onCheckedChange={(value) => (rememberCredential = value)} aria-label="Remember credential in native keyring" />
			</Field>

			{#if pending}
				{@const confirmation = pending}
				<Alert variant="destructive">
					<ShieldAlert />
					<AlertTitle>Confirm tunnel target</AlertTitle>
					<AlertDescription>
						<p>Your Argo CD credential will be sent to this workload. A workload behind this Service can receive it.</p>
						<dl class="my-2 grid grid-cols-[auto_1fr] gap-x-3 font-mono text-xs">
							<dt>Namespace</dt><dd>{confirmation.preview.namespace}</dd>
							<dt>Service</dt><dd>{confirmation.preview.serviceName}</dd>
							<dt>Pod</dt><dd>{confirmation.preview.podName}</dd>
							{#if confirmation.preview.certificateSha256}
								<dt>Certificate</dt><dd class="break-all">SHA-256 {formatCertificateSha256(confirmation.preview.certificateSha256)}</dd>
							{/if}
						</dl>
						{#if !confirmation.argoLabeled}
							<p class="font-medium">This Service does not carry the standard Argo CD label (app.kubernetes.io/part-of=argocd). Continue only if you recognise it as your Argo CD server.</p>
						{/if}
						{#if confirmation.preview.certificateSha256}
							<p>Only this certificate is accepted later. If Argo CD replaces it, you confirm again.</p>
						{/if}
						<div class="mt-2 flex gap-2">
							<Button
								type="button"
								size="sm"
								disabled={busy !== null}
								onclick={() =>
									connect(confirmation.endpoint, confirmation.saved, {
										namespace: confirmation.preview.namespace,
										serviceName: confirmation.preview.serviceName,
										podName: confirmation.preview.podName,
										certificateSha256: confirmation.preview.certificateSha256,
									})}>Confirm and connect</Button
							>
							<Button type="button" size="sm" variant="outline" onclick={() => (pending = null)}>Cancel</Button>
						</div>
					</AlertDescription>
				</Alert>
			{/if}
			{#if error}
				<Alert variant="destructive">
					<AlertCircle />
					<AlertTitle>Connection failed</AlertTitle>
					<AlertDescription>
						<p>{error}</p>
						{#if reconfirm}
							{@const profile = reconfirm}
							<Button class="mt-2" size="sm" variant="outline" type="button" onclick={() => startConnect(profile.endpoint, profile)}>Confirm target again</Button>
						{/if}
					</AlertDescription>
				</Alert>
			{/if}
			<Button type="button" disabled={busy !== null || !endpoint || !hasCredential} onclick={() => endpoint && startConnect(endpoint)}>
				<Link2 />{busy ?? "Connect"}
			</Button>
		</FieldGroup>
	</DialogContent>
</Dialog>
