<script lang="ts">
	import "@xterm/xterm/css/xterm.css";

	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { FitAddon } from "@xterm/addon-fit";
	import { Terminal as XtermTerminal } from "@xterm/xterm";
	import { onMount, tick } from "svelte";
	import { Play, RotateCcw, Square, Terminal as TerminalIcon } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Badge,
		Button,
		Checkbox,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Field,
		FieldDescription,
		FieldGroup,
		FieldLabel,
		Label,
		Select,
		SelectContent,
		SelectGroup,
		SelectItem,
		SelectTrigger,
		SelectValue,
		Spinner,
		Textarea,
	} from "@/components/ui/svelte";
	import {
		buildPodExecRequest,
		commandForPreset,
		invalidatePodExecQueries,
		isPodExecForResource,
		podExecCommandText,
		podExecQueryOptions,
		podExecTarget,
		startPodExec,
		stopPodExec,
		type PodExecPreset,
	} from "@/features/live-sessions";
	import { settingsStore } from "@/lib/settings-store";
	import {
		closePodExecChannel,
		createPodExecChannel,
		resizePodExecTerminal,
		writePodExecStdin,
		type TauriClient,
	} from "@/lib/tauri";
	import type {
		PodExecSessionMessage,
		PodExecSessionSummary,
		ResourceSummary,
	} from "@/lib/types";
	import type { ContainerStatusRow } from "./helpers";

	const DEFAULT_COLS = 100;
	const DEFAULT_ROWS = 32;

	let {
		client,
		resource,
		containers,
		selectedContainer = $bindable(""),
		kubeconfigSourceKey,
		active,
	}: {
		client: TauriClient;
		resource: ResourceSummary;
		containers: ContainerStatusRow[];
		selectedContainer?: string;
		kubeconfigSourceKey?: string;
		active: boolean;
	} = $props();

	const queryClient = useQueryClient();
	let preset = $state<PodExecPreset>("sh");
	let customArgv = $state("");
	let confirmed = $state(false);
	let starting = $state(false);
	let stoppingId = $state<string | null>(null);
	let status = $state("idle");
	let message = $state("Start an exec session to open the terminal.");
	let error = $state<unknown>(null);
	let sessionId = $state<string | null>(null);
	let channel = $state<ReturnType<typeof createPodExecChannel> | null>(null);
	let terminalHost = $state<HTMLDivElement | null>(null);
	let terminal: XtermTerminal | null = null;
	let fitAddon: FitAddon | null = null;

	const isPod = $derived(resource.kind === "Pod" && Boolean(resource.namespace));
	const showKubeconfigSourceLabels = $derived(
		$settingsStore.showKubeconfigSourceLabels,
	);
	const containerOptions = $derived(
		containers.filter((container) => container.type !== "init").length > 0
			? containers.filter((container) => container.type !== "init")
			: containers,
	);
	const command = $derived(commandForPreset(preset, customArgv));
	const commandText = $derived(typeof command === "string" ? "" : podExecCommandText(command));

	const sessionsQuery = createQuery<PodExecSessionSummary[]>(() =>
		podExecQueryOptions(client, {
			enabled: active,
			refetchInterval: active ? 3_000 : false,
		}),
	);
	const sessions = $derived(
		(sessionsQuery.data ?? []).filter((session) =>
			isPodExecForResource(session, resource, kubeconfigSourceKey),
		),
	);

	onMount(() => {
		if (terminalHost) {
			const nextTerminal = new XtermTerminal({
				cursorBlink: true,
				convertEol: true,
				fontFamily:
					"var(--font-mono, ui-monospace, SFMono-Regular, Consolas, monospace)",
				fontSize: 12,
				theme: {
					background: "#101113",
					foreground: "#f4f4f5",
				},
			});
			const nextFitAddon = new FitAddon();
			nextTerminal.loadAddon(nextFitAddon);
			nextTerminal.open(terminalHost);
			nextTerminal.onData((data) => {
				if (sessionId) void writePodExecStdin(client, sessionId, data);
			});
			nextTerminal.onResize(({ cols, rows }) => {
				if (sessionId) void resizePodExecTerminal(client, sessionId, { cols, rows });
			});
			terminal = nextTerminal;
			fitAddon = nextFitAddon;
			const fit = () => fitTerminal();
			const frame = window.requestAnimationFrame(fit);
			window.addEventListener("resize", fit);
			return () => {
				window.cancelAnimationFrame(frame);
				window.removeEventListener("resize", fit);
				if (channel) closePodExecChannel(channel);
				if (sessionId) {
					void stopPodExec({
						client,
						sessionId,
						invalidateQueries: (options) => queryClient.invalidateQueries(options),
					});
				}
				nextTerminal.dispose();
				terminal = null;
				fitAddon = null;
			};
		}
		return () => {
			if (channel) closePodExecChannel(channel);
			if (sessionId) {
				void stopPodExec({
					client,
					sessionId,
					invalidateQueries: (options) => queryClient.invalidateQueries(options),
				});
			}
		};
	});

	$effect(() => {
		if (!active) return;
		const frame = window.requestAnimationFrame(() => fitTerminal());
		return () => window.cancelAnimationFrame(frame);
	});

	function fitTerminal() {
		try {
			fitAddon?.fit();
		} catch {
			// Terminal can be hidden while tab/panel layout settles.
		}
	}

	function appendOutput(text: string) {
		terminal?.write(text);
	}

	function closeChannelOnly() {
		if (channel) closePodExecChannel(channel);
		channel = null;
		sessionId = null;
	}

	function handleMessage(event: PodExecSessionMessage) {
		if (event.type !== "started" && sessionId && event.sessionId !== sessionId) return;
		if (event.type === "started") {
			sessionId = event.sessionId;
			status = event.summary.status;
			return;
		}
		if (event.type === "status") {
			status = event.status;
			message = event.message;
			return;
		}
		if (event.type === "output") {
			appendOutput(event.data);
			return;
		}
		if (event.type === "error") {
			status = "error";
			error = event.message;
			appendOutput(`\n${event.message}\n`);
			closeChannelOnly();
			void invalidatePodExecQueries((options) => queryClient.invalidateQueries(options));
			return;
		}
		if (event.type === "exited") {
			status = "exited";
			message =
				event.exitCode === undefined
					? "Exec session exited"
					: `Exec session exited with code ${event.exitCode}`;
			closeChannelOnly();
			void invalidatePodExecQueries((options) => queryClient.invalidateQueries(options));
			return;
		}
		if (event.type === "stopped") {
			message = "Exec session stopped";
			closeChannelOnly();
			void invalidatePodExecQueries((options) => queryClient.invalidateQueries(options));
		}
	}

	async function startSession() {
		error = null;
		await tick();
		fitTerminal();
		const request = buildPodExecRequest(
			resource,
			{
				preset,
				customArgv,
				container: selectedContainer,
				cols: terminal?.cols ?? DEFAULT_COLS,
				rows: terminal?.rows ?? DEFAULT_ROWS,
				confirmed,
			},
			kubeconfigSourceKey,
		);
		if (typeof request === "string") {
			error = request;
			return;
		}
		const previous = sessionId;
		closeChannelOnly();
		if (previous) {
			await stopPodExec({
				client,
				sessionId: previous,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
		}
		const nextChannel = createPodExecChannel(handleMessage);
		channel = nextChannel;
		starting = true;
		status = "starting";
		message = "Starting exec session";
		terminal?.clear();
		terminal?.writeln(
			`Starting ${commandText} on ${podExecTarget(resource, selectedContainer)}\r\n`,
		);
		try {
			const summary = await startPodExec({
				client,
				request,
				channel: nextChannel,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
			sessionId = summary.id;
			status = summary.status;
		} catch (err) {
			closePodExecChannel(nextChannel);
			if (channel === nextChannel) channel = null;
			status = "error";
			error = err;
		} finally {
			starting = false;
		}
	}

	async function stopSession(id: string) {
		stoppingId = id;
		try {
			if (sessionId === id) closeChannelOnly();
			await stopPodExec({
				client,
				sessionId: id,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
		} catch (err) {
			error = err;
		} finally {
			stoppingId = null;
		}
	}
</script>

{#if resource.kind !== "Pod"}
	<Empty class="min-h-32 border border-dashed">
		<EmptyHeader>
			<EmptyTitle>Exec starts from Pods</EmptyTitle>
			<EmptyDescription>Select an exact namespaced Pod before exec.</EmptyDescription>
		</EmptyHeader>
	</Empty>
{:else if !isPod}
	<Alert variant="destructive">
		<AlertTitle>Namespace required</AlertTitle>
		<AlertDescription>Pod exec requires a namespaced Pod target.</AlertDescription>
	</Alert>
{:else}
	<div class="flex flex-col gap-3">
		<Alert>
			<TerminalIcon />
			<AlertTitle>Guarded Pod exec</AlertTitle>
			<AlertDescription>Exec is limited to this exact Pod through the Kubernetes API.</AlertDescription>
		</Alert>

		<FieldGroup>
			<Field>
				<FieldLabel>Container</FieldLabel>
				<Select
					value={selectedContainer || "__default"}
					items={[
						{ value: "__default", label: "Default container" },
						...containerOptions.map((container) => ({
							value: container.name,
							label: container.name,
						})),
					]}
					onValueChange={(value: string) => {
						selectedContainer = value === "__default" ? "" : value;
						confirmed = false;
					}}
				>
					<SelectTrigger class="w-full">
						<SelectValue placeholder="Default container" />
					</SelectTrigger>
					<SelectContent>
						<SelectGroup>
							<SelectItem value="__default">Default container</SelectItem>
							{#each containerOptions as container (`${container.type}:${container.name}`)}
								<SelectItem value={container.name}>{container.name}</SelectItem>
							{/each}
						</SelectGroup>
					</SelectContent>
				</Select>
				<FieldDescription>Kubernetes chooses default only when no container is selected.</FieldDescription>
			</Field>
			<Field>
				<FieldLabel>Command</FieldLabel>
				<Select
					value={preset}
					items={[
						{ value: "sh", label: "/bin/sh" },
						{ value: "bash", label: "/bin/bash" },
						{ value: "custom", label: "Custom argv" },
					]}
					onValueChange={(value: string) => {
						preset = value as PodExecPreset;
						confirmed = false;
					}}
				>
					<SelectTrigger class="w-full">
						<SelectValue />
					</SelectTrigger>
					<SelectContent>
						<SelectGroup>
							<SelectItem value="sh">/bin/sh</SelectItem>
							<SelectItem value="bash">/bin/bash</SelectItem>
							<SelectItem value="custom">Custom argv</SelectItem>
						</SelectGroup>
					</SelectContent>
				</Select>
				<FieldDescription>Presets are exact commands; no local shell parsing.</FieldDescription>
			</Field>
			{#if preset === "custom"}
				<Field>
					<FieldLabel>Custom argv</FieldLabel>
					<Textarea
						bind:value={customArgv}
						placeholder={"/usr/bin/env\nprintenv"}
						oninput={() => (confirmed = false)}
					/>
					<FieldDescription>One argv item per line.</FieldDescription>
				</Field>
			{/if}
		</FieldGroup>

		<div class="rounded-md border bg-muted/20 p-3 text-xs">
			<div class="font-medium">Target</div>
			<div class="mt-1 break-words font-mono text-muted-foreground">
				{podExecTarget(resource, selectedContainer)}
			</div>
			{#if showKubeconfigSourceLabels && kubeconfigSourceKey}
				<div class="mt-2 font-medium">Kubeconfig source</div>
				<div class="mt-1 break-words font-mono text-muted-foreground">{kubeconfigSourceKey}</div>
			{/if}
			<div class="mt-3 font-medium">Command</div>
			<div class="mt-1 break-words font-mono text-muted-foreground">
				{commandText || "Custom argv is required"}
			</div>
		</div>

		<Label class="gap-2 rounded-md border bg-background p-3 text-xs text-muted-foreground">
			<Checkbox checked={confirmed} onCheckedChange={(checked) => (confirmed = checked)} />
			I understand this opens an interactive session in the selected Pod.
		</Label>

		{#if error}
			<FriendlyError
				error={error}
				context={{ operation: "exec", fallbackTitle: "Exec failed" }}
			/>
		{/if}

		<div class="flex flex-wrap items-center gap-2">
			<Badge variant={status === "error" ? "destructive" : "outline"}>Exec: {status}</Badge>
			<span class="text-xs text-muted-foreground">{message}</span>
			<Button onclick={startSession} disabled={starting}>
				{#if starting}
					<Spinner data-icon="inline-start" />
				{:else}
					<Play data-icon="inline-start" />
				{/if}
				Start
			</Button>
		</div>

		<div
			bind:this={terminalHost}
			class="min-h-[280px] overflow-hidden rounded-md border bg-background p-2"
			class:hidden={!active}
		></div>
		<div class="flex justify-end">
			<Button variant="outline" size="sm" onclick={() => terminal?.clear()}>
				<RotateCcw data-icon="inline-start" />
				Clear terminal
			</Button>
		</div>

		<div class="flex items-center justify-between gap-2">
			<div class="text-xs font-semibold uppercase text-muted-foreground">Active exec sessions</div>
			{#if sessionsQuery.isFetching}<Spinner />{/if}
		</div>
		{#if sessions.length === 0}
			<Empty class="min-h-24 border border-dashed">
				<EmptyHeader>
					<EmptyTitle>No exec sessions</EmptyTitle>
					<EmptyDescription>Start guarded exec for this Pod.</EmptyDescription>
				</EmptyHeader>
			</Empty>
		{:else}
			<div class="flex flex-col gap-2">
				{#each sessions as item (item.id)}
					<div class="flex flex-wrap items-center justify-between gap-2 rounded-md border bg-background p-3">
						<div class="min-w-0">
							<div class="truncate font-mono text-xs">{podExecCommandText(item.command)}</div>
							<div class="text-[11px] text-muted-foreground">
								{item.container ? `Container ${item.container}` : "Default container"}
							</div>
							{#if showKubeconfigSourceLabels && item.kubeconfigSourceLabel}
								<div class="truncate text-[11px] text-muted-foreground">
									{item.kubeconfigSourceLabel}
								</div>
							{/if}
						</div>
						<div class="flex items-center gap-2">
							<Badge variant={item.status === "error" ? "destructive" : "outline"}>{item.status}</Badge>
							<Button
								variant="outline"
								size="sm"
								onclick={() => stopSession(item.id)}
								disabled={stoppingId === item.id}
							>
								{#if stoppingId === item.id}
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
