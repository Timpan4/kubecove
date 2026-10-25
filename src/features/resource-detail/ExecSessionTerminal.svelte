<script lang="ts">
	import { useQueryClient } from "@tanstack/svelte-query";
	import { onMount, tick } from "svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		buildPodExecRequest,
		invalidatePodExecQueries,
		podExecCommandLabel,
		podExecTarget,
		startPodExec,
		stopPodExec,
	} from "@/features/live-sessions";
	import {
		closePodExecChannel,
		createPodExecChannel,
		resizePodExecTerminal,
		writePodExecStdin,
		type TauriClient,
	} from "@/lib/tauri";
	import type {
		PodExecSessionMessage,
		PodExecSessionStatus,
		ResourceSummary,
	} from "@/lib/types";
	import { createExecInput } from "./execInput";
	import { createExecTerminal, type ExecTerminalHandle } from "./execTerminal";

	const DEFAULT_COLS = 100;
	const DEFAULT_ROWS = 32;
	const DIM = "\x1b[2m";
	const RESET = "\x1b[0m";

	let {
		client,
		resource,
		kubeconfigSourceKey,
		container,
		command,
		visible,
		sessionId = $bindable(null),
		status = $bindable("starting"),
	}: {
		client: TauriClient;
		resource: ResourceSummary;
		kubeconfigSourceKey?: string;
		container: string;
		command: readonly string[];
		visible: boolean;
		/** Set only while the backend session is live. */
		sessionId?: string | null;
		status?: PodExecSessionStatus | "stopped";
	} = $props();

	const queryClient = useQueryClient();
	const invalidateQueries = (options: { queryKey: readonly unknown[] }) =>
		queryClient.invalidateQueries(options);
	let error = $state<unknown>(null);
	let host = $state<HTMLDivElement | null>(null);
	let terminal: ExecTerminalHandle | null = null;
	let input: ReturnType<typeof createExecInput> | null = null;
	let channel: ReturnType<typeof createPodExecChannel> | null = null;
	let starting = false;
	let fitFrame: number | null = null;

	export function clear() {
		terminal?.clear();
	}

	export function restart() {
		void start();
	}

	export function stop() {
		void stopSession();
	}

	onMount(() => {
		if (!host) return;
		terminal = createExecTerminal(
			host,
			(data) => input?.enqueue(data),
			(size) => {
				if (sessionId) {
					void resizePodExecTerminal(client, sessionId, size).catch((cause) => {
						error = cause;
					});
				}
			},
		);
		const observer = new ResizeObserver(fitSoon);
		observer.observe(host);
		void start();
		return () => {
			observer.disconnect();
			if (fitFrame !== null) window.cancelAnimationFrame(fitFrame);
			const liveSessionId = sessionId;
			closeChannel();
			if (liveSessionId) void stopPodExec({ client, sessionId: liveSessionId, invalidateQueries });
			terminal?.dispose();
			terminal = null;
		};
	});

	$effect(() => {
		if (visible) fitSoon();
	});

	function fitSoon() {
		fitFrame ??= window.requestAnimationFrame(() => {
			fitFrame = null;
			terminal?.fit();
		});
	}

	function closeChannel() {
		input?.dispose();
		input = null;
		if (channel) closePodExecChannel(channel);
		channel = null;
		sessionId = null;
	}

	function ended(nextStatus: PodExecSessionStatus | "stopped", note: string) {
		status = nextStatus;
		terminal?.writeln(`\r\n${DIM}[${note}]${RESET}`);
		closeChannel();
		void invalidatePodExecQueries(invalidateQueries);
	}

	function handleMessage(event: PodExecSessionMessage) {
		if (event.type !== "started" && sessionId && event.sessionId !== sessionId) return;
		switch (event.type) {
			case "started":
				sessionId = event.sessionId;
				input?.dispose();
				input = createExecInput(
					(data) => writePodExecStdin(client, event.sessionId, data),
					(cause) => {
						error = cause;
					},
				);
				status = event.summary.status;
				return;
			case "status":
				status = event.status;
				return;
			case "output":
				terminal?.write(event.data);
				return;
			case "error":
				error = event.message;
				ended("error", event.message);
				return;
			case "exited":
				ended("exited", event.exitCode === undefined ? "exited" : `exited with code ${event.exitCode}`);
				return;
			case "stopped":
				ended("stopped", "session stopped");
		}
	}

	async function start() {
		if (starting) return;
		starting = true;
		try {
			error = null;
			const previous = sessionId;
			closeChannel();
			if (previous) await stopPodExec({ client, sessionId: previous, invalidateQueries });
			await tick();
			terminal?.fit();
			const built = buildPodExecRequest(
				resource,
				{
					command,
					container,
					cols: terminal?.cols ?? DEFAULT_COLS,
					rows: terminal?.rows ?? DEFAULT_ROWS,
				},
				kubeconfigSourceKey,
			);
			if (!built.ok) {
				status = "error";
				error = built.message;
				return;
			}
			const { request } = built;
			const nextChannel = createPodExecChannel(handleMessage, client);
			channel = nextChannel;
			status = "starting";
			terminal?.writeln(
				`${DIM}Starting ${podExecCommandLabel(command)} on ${podExecTarget(resource, container)}${RESET}\r\n`,
			);
			try {
				const summary = await startPodExec({ client, request, channel: nextChannel, invalidateQueries });
				// Channel messages may have advanced or torn down the session
				// while the start call was in flight; never regress their state.
				if (channel !== nextChannel) return;
				if (!sessionId) {
					sessionId = summary.id;
					status = summary.status;
				}
			} catch (cause) {
				closePodExecChannel(nextChannel);
				if (channel === nextChannel) channel = null;
				status = "error";
				error = cause;
			}
		} finally {
			starting = false;
		}
	}

	async function stopSession() {
		const liveSessionId = sessionId;
		if (!liveSessionId) return;
		ended("stopped", "session stopped");
		try {
			await stopPodExec({ client, sessionId: liveSessionId, invalidateQueries });
		} catch (cause) {
			error = cause;
		}
	}
</script>

<div class="absolute inset-0 flex flex-col" class:hidden={!visible}>
	{#if error}
		<div class="shrink-0 p-2">
			<FriendlyError {error} context={{ operation: "exec", fallbackTitle: "Exec failed" }} />
		</div>
	{/if}
	<div bind:this={host} class="min-h-0 flex-1 px-2.5 pt-2"></div>
</div>
