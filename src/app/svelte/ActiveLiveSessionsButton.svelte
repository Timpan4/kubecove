<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { onMount } from "svelte";
	import { Cable, Copy, RotateCcw, Square, Terminal } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Badge,
		Button,
		Popover,
		PopoverContent,
		PopoverTrigger,
		Separator,
		Spinner,
		buttonClass,
	} from "@/components/ui/svelte";
	import { buildLiveSessionReadModel } from "@/features/live-sessions/liveSessionReadModel";
	import { refreshLiveSessionQueries, trackLiveSessionUi } from "@/features/live-sessions/sessionPolling.svelte";
	import { openExecSessions, showOpenExecSession } from "@/features/live-sessions/openExecSessions.svelte";
	import { podExecCommandText } from "@/features/live-sessions/podExecHelpers";
	import { podExecQueryOptions, stopPodExec } from "@/features/live-sessions/podExecLifecycle";
	import { liveSessionBadgeVariant, portForwardLocalUrl } from "@/features/live-sessions/helpers";
	import { LiveSessionRowActions } from "@/features/live-sessions/rowActions.svelte";
	import {
		portForwardQueryOptions,
		reconnectPortForward as reconnectPortForwardLifecycle,
		stopPortForward as stopPortForwardLifecycle,
	} from "@/features/live-sessions/portForwardLifecycle";
	import { settingsStore } from "@/lib/settings-store";
	import { createTauriClient } from "@/lib/tauri";
	import type { PodExecSessionSummary, PortForwardSessionSummary } from "@/lib/types";

	let { onOpenManager }: { onOpenManager: () => void } = $props();

	const client = createTauriClient();
	const queryClient = useQueryClient();
	let popoverOpen = $state(false);
	const rows = new LiveSessionRowActions();
	const portForwardsQuery = createQuery(() => portForwardQueryOptions(client));
	const execSessionsQuery = createQuery(() => podExecQueryOptions(client));
	trackLiveSessionUi(() => popoverOpen);
	onMount(() => {
		const onVisible = () => {
			if (document.visibilityState === "visible") refreshLiveSessionQueries(queryClient);
		};
		document.addEventListener("visibilitychange", onVisible);
		return () => document.removeEventListener("visibilitychange", onVisible);
	});

	const liveSessionModel = $derived(
		buildLiveSessionReadModel(
			portForwardsQuery.data ?? [],
			execSessionsQuery.data ?? [],
		),
	);
	const portForwardSessions = $derived(liveSessionModel.portForwards);
	const execSessions = $derived(liveSessionModel.podExecSessions);
	const showableExecSessionIds = $derived(new Set(openExecSessions().map((session) => session.id)));
	const sessionCount = $derived(liveSessionModel.counts.total);
	const loading = $derived(portForwardsQuery.isLoading || execSessionsQuery.isLoading);
	const fetching = $derived(portForwardsQuery.isFetching || execSessionsQuery.isFetching);
	const showKubeconfigSourceLabels = $derived(
		$settingsStore.showKubeconfigSourceLabels,
	);

	function sessionLabel(session: PortForwardSessionSummary): string {
		return `${session.namespace}/${session.targetKind}/${session.targetName}:${session.remotePort}`;
	}

	function sessionResolution(session: PortForwardSessionSummary): string {
		return `${session.resolvedPodName}:${session.resolvedPodPort}`;
	}

	function execSessionLabel(session: PodExecSessionSummary): string {
		return `${session.namespace}/Pod/${session.podName}`;
	}

	function stopPortForward(sessionId: string): Promise<boolean> {
		return rows.run(sessionId, "stop", () =>
			stopPortForwardLifecycle({
				client,
				sessionId,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			}),
		);
	}

	function showExecSession(sessionId: string) {
		popoverOpen = false;
		showOpenExecSession(sessionId);
	}

	function stopExecSession(sessionId: string): Promise<boolean> {
		return rows.run(sessionId, "stop", () =>
			stopPodExec({
				client,
				sessionId,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			}),
		);
	}

	function reconnectPortForward(session: PortForwardSessionSummary): Promise<boolean> {
		return rows.run(session.id, "reconnect", () =>
			reconnectPortForwardLifecycle({
				client,
				session,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			}),
		);
	}

	function copySessionUrl(session: PortForwardSessionSummary): Promise<boolean> {
		return rows.run(session.id, "copy", async () => {
			await navigator.clipboard?.writeText(portForwardLocalUrl(session));
		});
	}

	function openManager() {
		popoverOpen = false;
		onOpenManager();
	}
</script>

{#if sessionCount > 0 || loading}
	<Popover bind:open={popoverOpen}>
		<PopoverTrigger
			type="button"
			class={buttonClass({
				variant: "ghost",
				size: "sm",
				className: "mr-1 h-8 text-muted-foreground [-webkit-app-region:no-drag]",
			})}
			aria-label={`Open live sessions (${sessionCount} active)`}
		>
			{#if fetching}
				<Spinner class="size-3.5" />
			{:else}
				<Cable data-icon="inline-start" />
			{/if}
			{sessionCount}
		</PopoverTrigger>
		<PopoverContent align="end" class="w-96 max-w-[calc(100vw-1rem)]">
			<div class="flex flex-col gap-3">
				<div class="flex items-center justify-between gap-3">
					<div class="min-w-0">
						<div class="truncate text-sm font-semibold">Live sessions</div>
						<div class="text-xs text-muted-foreground">
							Active port-forwards and Pod exec sessions
						</div>
					</div>
					{#if fetching}
						<Spinner class="size-3.5" />
					{/if}
				</div>
				<Button type="button" variant="outline" size="sm" onclick={openManager}>
					<Cable data-icon="inline-start" />
					Manage
				</Button>
				{#if rows.failure}
					<FriendlyError
						mode="compact"
						error={rows.failure.error}
						context={{ operation: "liveSession", fallbackTitle: "Live-session action failed" }}
					/>
				{/if}
				<Separator />
				<div class="flex max-h-80 flex-col gap-2 overflow-y-auto">
					{#each execSessions as session (session.id)}
						<div class="flex min-w-0 flex-col gap-2 rounded-md border bg-background p-2">
							<div class="flex min-w-0 items-center justify-between gap-2">
								<div class="min-w-0">
									<div class="inline-flex max-w-full items-center gap-1 truncate text-xs font-medium">
										<Terminal class="size-3 shrink-0" />
										<span class="truncate">{execSessionLabel(session)}</span>
									</div>
									<div class="truncate font-mono text-xs text-muted-foreground">
										{podExecCommandText(session.command)}
									</div>
									{#if showKubeconfigSourceLabels && session.kubeconfigSourceLabel}
										<div class="truncate text-xs text-muted-foreground">
											{session.kubeconfigSourceLabel}
										</div>
									{/if}
								</div>
								<Badge variant={liveSessionBadgeVariant(session.status)}>
									{session.status}
								</Badge>
							</div>
							{#if session.lastError}
								<FriendlyError
									mode="compact"
									error={session.lastError}
									context={{
										operation: "exec",
										fallbackTitle: "Exec session failed",
										partial: true,
									}}
								/>
							{/if}
							<div class="flex flex-wrap justify-end gap-2">
								{#if showableExecSessionIds.has(session.id)}
									<Button
										type="button"
										variant="outline"
										size="sm"
										aria-label={`Show exec session in ${session.podName}`}
										onclick={() => showExecSession(session.id)}
									>
										<Terminal data-icon="inline-start" />
										Show
									</Button>
								{/if}
								<Button
									type="button"
									variant="outline"
									size="sm"
									disabled={rows.pending(session.id) !== undefined}
									onclick={() => stopExecSession(session.id)}
								>
									{#if rows.pending(session.id) === "stop"}
										<Spinner data-icon="inline-start" />
									{:else}
										<Square data-icon="inline-start" />
									{/if}
									Stop
								</Button>
							</div>
						</div>
					{/each}
					{#each portForwardSessions as session (session.id)}
						{@const localUrl = portForwardLocalUrl(session)}
						<div class="flex min-w-0 flex-col gap-2 rounded-md border bg-background p-2">
							<div class="flex min-w-0 items-center justify-between gap-2">
								<div class="min-w-0">
									<div class="truncate text-xs font-medium">{sessionLabel(session)}</div>
									<div class="truncate font-mono text-xs text-muted-foreground">
										{localUrl}
									</div>
									<div class="truncate text-xs text-muted-foreground">
										Resolved {sessionResolution(session)}
									</div>
									{#if showKubeconfigSourceLabels && session.kubeconfigSourceLabel}
										<div class="truncate text-xs text-muted-foreground">
											{session.kubeconfigSourceLabel}
										</div>
									{/if}
								</div>
								<Badge variant={liveSessionBadgeVariant(session.status)}>
									{session.status}
								</Badge>
							</div>
							{#if session.lastError}
								<FriendlyError
									mode="compact"
									error={session.lastError}
									context={{
										operation: "portForward",
										fallbackTitle: "Port-forward failed",
										partial: true,
									}}
								/>
							{/if}
							<div class="flex flex-wrap justify-end gap-2">
								<Button
									type="button"
									variant="outline"
									size="sm"
									disabled={rows.pending(session.id) !== undefined}
									onclick={() => reconnectPortForward(session)}
								>
									{#if rows.pending(session.id) === "reconnect"}
										<Spinner data-icon="inline-start" />
									{:else}
										<RotateCcw data-icon="inline-start" />
									{/if}
									Reconnect
								</Button>
								<Button
									type="button"
									variant="outline"
									size="sm"
									disabled={rows.pending(session.id) !== undefined}
									onclick={() => copySessionUrl(session)}
								>
									<Copy data-icon="inline-start" />
									Copy
								</Button>
								<Button
									type="button"
									variant="outline"
									size="sm"
									disabled={rows.pending(session.id) !== undefined}
									onclick={() => stopPortForward(session.id)}
								>
									{#if rows.pending(session.id) === "stop"}
										<Spinner data-icon="inline-start" />
									{:else}
										<Square data-icon="inline-start" />
									{/if}
									Stop
								</Button>
							</div>
						</div>
					{/each}
					{#if sessionCount === 0}
						<div class="text-sm text-muted-foreground">Loading active sessions...</div>
					{/if}
				</div>
			</div>
		</PopoverContent>
	</Popover>
{/if}
