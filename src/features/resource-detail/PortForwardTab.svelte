<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { Cable, Copy, Play, Square } from "lucide-svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Badge,
		Button,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Field,
		FieldDescription,
		FieldGroup,
		FieldLabel,
		Input,
		Select,
		SelectContent,
		SelectGroup,
		SelectItem,
		SelectTrigger,
		SelectValue,
		Spinner,
	} from "@/components/ui/svelte";
	import {
		isPortForwardForResource,
		extractServicePortOptions,
		parsePortForwardForm,
		portForwardLocalUrl,
		sortPortForwardSessions,
	} from "@/features/live-sessions/helpers";
	import { workspaceStore } from "@/features/workspaces/workspaceStore";
	import { queryKeys } from "@/lib/queryKeys";
	import { settingsStore } from "@/lib/settings-store";
	import {
		listPortForwards,
		startPodPortForward,
		stopPodPortForward,
		type TauriClient,
	} from "@/lib/tauri";
	import type {
		PortForwardSessionSummary,
		ResourceSummary,
	} from "@/lib/types";
	import { workspaceScopeContexts } from "@/lib/workspace-model";
	import { getErrorMessage } from "./helpers";

	let {
		client,
		resource,
		kubeconfigSourceKey,
		yaml,
		active,
	}: {
		client: TauriClient;
		resource: ResourceSummary;
		kubeconfigSourceKey?: string;
		yaml?: string;
		active: boolean;
	} = $props();

	const queryClient = useQueryClient();
	const selectedWorkspace = workspaceStore.selectedWorkspace;
	const showKubeconfigSourceLabels = $derived(
		$settingsStore.showKubeconfigSourceLabels,
	);
	let remotePort = $state("");
	let localPort = $state("");
	let starting = $state(false);
	let stoppingId = $state<string | null>(null);
	let copyingId = $state<string | null>(null);
	let error = $state("");
	let copyError = $state("");
	let saveMessage = $state("");
	let saveError = $state("");

	const targetSupported = $derived(
		(resource.kind === "Pod" || resource.kind === "Service") && Boolean(resource.namespace),
	);
	const servicePorts = $derived(extractServicePortOptions(yaml));
	const sessionsQuery = createQuery<PortForwardSessionSummary[]>(() => ({
		queryKey: queryKeys.portForwards(),
		queryFn: () => listPortForwards(client),
		enabled: active,
		refetchInterval: active ? 3_000 : false,
	}));
	const sessions = $derived(
		sortPortForwardSessions(
			(sessionsQuery.data ?? []).filter((session) =>
				isPortForwardForResource(session, resource, kubeconfigSourceKey),
			),
		),
	);

	$effect(() => {
		if (resource.kind === "Service" && !remotePort && servicePorts[0]) {
			remotePort = String(servicePorts[0].port);
		}
	});

	async function startForward() {
		error = "";
		copyError = "";
		saveError = "";
		const parsed = parsePortForwardForm(
			{ remotePort, localPort },
			{ remotePortLabel: resource.kind === "Service" ? "Service port" : "Pod port" },
		);
		if (typeof parsed === "string") {
			error = parsed;
			return;
		}
		if (!resource.namespace || !targetSupported) {
			error = "Pod or Service target with namespace is required";
			return;
		}
		starting = true;
		try {
			await startPodPortForward(client, {
				clusterContext: resource.cluster,
				kubeconfigEnvVar: kubeconfigSourceKey,
				namespace: resource.namespace,
				targetKind: resource.kind === "Service" ? "Service" : "Pod",
				targetName: resource.name,
				remotePort: parsed.remotePort,
				localPort: parsed.localPort,
			});
			localPort = "";
			await queryClient.invalidateQueries({ queryKey: queryKeys.portForwards() });
		} catch (err) {
			error = getErrorMessage(err);
		} finally {
			starting = false;
		}
	}

	function saveServicePreset() {
		error = "";
		saveMessage = "";
		saveError = "";
		if (resource.kind !== "Service") return;
		const activeWorkspace = $selectedWorkspace;
		if (!activeWorkspace) {
			saveError = "Open a workspace before saving port-forward presets.";
			return;
		}
		const parsed = parsePortForwardForm(
			{ remotePort, localPort },
			{ remotePortLabel: "Service port" },
		);
		if (typeof parsed === "string") {
			error = parsed;
			return;
		}
		if (!workspaceScopeContexts(activeWorkspace.scope).includes(resource.cluster)) {
			saveError = "Workspace context must include this Service before saving a preset.";
			return;
		}
		const duplicate = (activeWorkspace.portForwards ?? []).some(
			(portForward) =>
				portForward.clusterContext === resource.cluster &&
				portForward.namespace === resource.namespace &&
				portForward.serviceName === resource.name &&
				portForward.servicePort === parsed.remotePort &&
				portForward.localPort === parsed.localPort,
		);
		if (duplicate) {
			saveMessage = "Preset already saved for this Service.";
			return;
		}
		workspaceStore.saveSavedPortForward(activeWorkspace.id, {
			clusterContext: resource.cluster,
			namespace: resource.namespace ?? "",
			serviceName: resource.name,
			servicePort: parsed.remotePort,
			localPort: parsed.localPort,
		});
		saveMessage = "Saved Service forward to this workspace.";
	}

	async function stopForward(id: string) {
		copyError = "";
		stoppingId = id;
		try {
			await stopPodPortForward(client, id);
			await queryClient.invalidateQueries({ queryKey: queryKeys.portForwards() });
		} catch (err) {
			error = getErrorMessage(err);
		} finally {
			stoppingId = null;
		}
	}

	async function copySessionUrl(session: PortForwardSessionSummary) {
		copyingId = session.id;
		copyError = "";
		try {
			await navigator.clipboard?.writeText(portForwardLocalUrl(session));
		} catch (err) {
			copyError = getErrorMessage(err);
		} finally {
			copyingId = null;
		}
	}

	function sessionTitle(session: PortForwardSessionSummary): string {
		return `${session.localAddress}:${session.localPort} -> ${session.targetKind}/${session.targetName}:${session.remotePort}`;
	}

	function sessionResolution(session: PortForwardSessionSummary): string {
		if (session.targetKind === "Service") {
			return `Resolved to Pod/${session.resolvedPodName}:${session.resolvedPodPort}`;
		}
		return `Pod/${session.resolvedPodName}:${session.resolvedPodPort}`;
	}
</script>

{#if !targetSupported}
	<Empty class="min-h-32 border border-dashed">
		<EmptyHeader>
			<EmptyTitle>Port-forward unavailable</EmptyTitle>
			<EmptyDescription>Choose an exact namespaced Pod or Service target.</EmptyDescription>
		</EmptyHeader>
	</Empty>
{:else}
	<div class="flex flex-col gap-3">
		<Alert>
			<Cable />
			<AlertTitle>Guarded port-forward</AlertTitle>
			<AlertDescription>
				Target: {resource.namespace}/{resource.name} ({resource.kind})
			</AlertDescription>
		</Alert>

		<FieldGroup>
			{#if resource.kind === "Service" && servicePorts.length > 0}
				<Field>
					<FieldLabel>Service port</FieldLabel>
					<Select
						value={remotePort || String(servicePorts[0]?.port ?? "")}
						items={servicePorts.map((port) => ({
							value: String(port.port),
							label: port.name ? `${port.name} (${port.port})` : String(port.port),
						}))}
						onValueChange={(value: string) => (remotePort = value)}
					>
						<SelectTrigger class="w-full">
							<SelectValue placeholder="Service port" />
						</SelectTrigger>
						<SelectContent>
							<SelectGroup>
								{#each servicePorts as port (`${port.name ?? "port"}:${port.port}`)}
									<SelectItem value={String(port.port)}>
										{port.name ? `${port.name} (${port.port})` : port.port}
									</SelectItem>
								{/each}
							</SelectGroup>
						</SelectContent>
					</Select>
					<FieldDescription>TCP ports parsed from Service YAML.</FieldDescription>
				</Field>
			{:else}
				<Field>
					<FieldLabel>{resource.kind === "Service" ? "Service port" : "Pod port"}</FieldLabel>
					<Input bind:value={remotePort} inputmode="numeric" placeholder="8080" />
					<FieldDescription>Remote TCP port to forward.</FieldDescription>
				</Field>
			{/if}
			<Field>
				<FieldLabel>Local port</FieldLabel>
				<Input bind:value={localPort} inputmode="numeric" placeholder="auto" />
				<FieldDescription>Optional. Must be 1024 or higher when set.</FieldDescription>
			</Field>
		</FieldGroup>

		{#if error}
			<Alert variant="destructive">
				<AlertTitle>Port-forward failed</AlertTitle>
				<AlertDescription>{error}</AlertDescription>
			</Alert>
		{/if}
		{#if saveError}
			<Alert variant="destructive">
				<AlertTitle>Preset not saved</AlertTitle>
				<AlertDescription>{saveError}</AlertDescription>
			</Alert>
		{/if}
		{#if saveMessage}
			<Alert>
				<AlertTitle>Service preset</AlertTitle>
				<AlertDescription>{saveMessage}</AlertDescription>
			</Alert>
		{/if}
		{#if copyError}
			<Alert variant="destructive">
				<AlertTitle>Copy failed</AlertTitle>
				<AlertDescription>{copyError}</AlertDescription>
			</Alert>
		{/if}

		<div class="flex flex-wrap items-center gap-2">
			<Button onclick={startForward} disabled={starting}>
				{#if starting}
					<Spinner data-icon="inline-start" />
				{:else}
					<Play data-icon="inline-start" />
				{/if}
				Start
			</Button>
			{#if resource.kind === "Service"}
				<Button type="button" variant="outline" onclick={saveServicePreset}>Save preset</Button>
			{/if}
			<span class="text-xs text-muted-foreground">Local URL appears after session starts.</span>
		</div>

		<div class="flex items-center justify-between gap-2">
			<div class="text-xs font-semibold uppercase text-muted-foreground">Active port-forwards</div>
			{#if sessionsQuery.isFetching}<Spinner />{/if}
		</div>
		{#if sessions.length === 0}
			<Empty class="min-h-24 border border-dashed">
				<EmptyHeader>
					<EmptyTitle>No port-forwards</EmptyTitle>
					<EmptyDescription>Start a guarded session for this target.</EmptyDescription>
				</EmptyHeader>
			</Empty>
		{:else}
			<div class="flex flex-col gap-2">
				{#each sessions as session (session.id)}
					<div class="flex flex-wrap items-center justify-between gap-2 rounded-md border bg-background p-3">
						<div class="min-w-0">
							<div class="truncate text-xs font-medium">{sessionTitle(session)}</div>
							<div class="truncate font-mono text-xs text-muted-foreground">
								{portForwardLocalUrl(session)}
							</div>
							<div class="truncate text-xs text-muted-foreground">
								{sessionResolution(session)}
							</div>
							{#if showKubeconfigSourceLabels && session.kubeconfigSourceLabel}
								<div class="truncate text-xs text-muted-foreground">
									{session.kubeconfigSourceLabel}
								</div>
							{/if}
							{#if session.lastError}
								<div class="text-xs text-destructive">{session.lastError}</div>
							{/if}
						</div>
						<div class="flex items-center gap-2">
							<Badge variant={session.status === "error" ? "destructive" : "outline"}>
								{session.status}
							</Badge>
							<Button
								variant="outline"
								size="sm"
								onclick={() => copySessionUrl(session)}
								disabled={copyingId === session.id}
							>
								{#if copyingId === session.id}
									<Spinner data-icon="inline-start" />
								{:else}
									<Copy data-icon="inline-start" />
								{/if}
								Copy
							</Button>
							<Button
								variant="outline"
								size="sm"
								onclick={() => stopForward(session.id)}
								disabled={stoppingId === session.id}
							>
								{#if stoppingId === session.id}
									<Spinner data-icon="inline-start" />
								{:else}
									<Square data-icon="inline-start" />
								{/if}
								Stop
							</Button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
{/if}
