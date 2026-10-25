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
	let startTask: Promise<void> | null = null;
	// Bumped by stop and unmount so a start that has not opened its channel yet gives up.
	let startRun = 0;
	let fitFrame: number | null = null;

	export function clear() {
		terminal?.clear();
	}

	export function restart() {
		// A stopped start may still be settling; restart after it instead of joining it.
		void (startTask ?? Promise.resolve()).then(start);
	}

	/** Resolves false when the backend refused to stop the session. */
	export function stop(): Promise<boolean> {
		return stopSession();
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
			// A start still in flight sees the closed channel and stops its own session.
			startRun += 1;
			closeChannel();
			// No tab is left to show a failure; the live-sessions indicator still lists
			// any session the backend kept, with its own Stop.
			if (liveSessionId) {
				void stopPodExec({ client, sessionId: liveSessionId, invalidateQueries }).catch(() => undefined);
			}
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

	function start(): Promise<void> {
		startTask ??= runStart().finally(() => {
			startTask = null;
		});
		return startTask;
	}

	async function runStart() {
		const run = ++startRun;
		error = null;
		const previous = sessionId;
		closeChannel();
		if (previous && !(await stopBackendSession(previous))) return;
		await tick();
		if (run !== startRun) return;
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
		const nextChannel = createPodExecChannel(handleMessage, client);
		channel = nextChannel;
		status = "starting";
		terminal?.writeln(
			`${DIM}Starting ${podExecCommandLabel(command)} on ${podExecTarget(resource, container)}${RESET}\r\n`,
		);
		try {
			const summary = await startPodExec({ client, request: built.request, channel: nextChannel, invalidateQueries });
			if (channel !== nextChannel) {
				// The tab stopped, closed, or unmounted, or the session already ended,
				// while start was in flight. Stop it so no session runs without a tab.
				await stopBackendSession(summary.id);
				return;
			}
			if (!sessionId) {
				sessionId = summary.id;
				status = summary.status;
			}
		} catch (cause) {
			closePodExecChannel(nextChannel);
			// A tab that was stopped or closed meanwhile has no start error to show.
			if (channel !== nextChannel) return;
			channel = null;
			status = "error";
			error = cause;
		}
	}

	async function stopBackendSession(id: string): Promise<boolean> {
		try {
			await stopPodExec({ client, sessionId: id, invalidateQueries });
			return true;
		} catch (cause) {
			// The session may still run: keep tracking it so the leave dialog,
			// unmount, and Stop all try to stop it again.
			terminal?.writeln(`\r\n${DIM}[stop failed; the session may still be running]${RESET}`);
			sessionId = id;
			status = "error";
			error = cause;
			return false;
		}
	}

	async function stopSession(): Promise<boolean> {
		const liveSessionId = sessionId;
		const startInFlight = startTask;
		if (!liveSessionId && !startInFlight) return true;
		startRun += 1;
		ended("stopped", "session stopped");
		if (liveSessionId) return stopBackendSession(liveSessionId);
		// Still starting: the start gives up or, once its channel is closed, stops its own
		// session. The backend start returns right after registering, so this wait is short.
		await startInFlight;
		return sessionId === null;
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
