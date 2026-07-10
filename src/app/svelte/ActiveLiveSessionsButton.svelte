<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
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
	import {
		portForwardLocalUrl,
		portForwardQueryOptions,
		reconnectPortForward as reconnectPortForwardLifecycle,
		stopPortForward as stopPortForwardLifecycle,
	} from "@/features/live-sessions";
	import { podExecCommandText, sortPodExecSessions } from "@/features/resource-detail/pod-exec-helpers";
	import { settingsStore } from "@/lib/settings-store";
	import {
		createTauriClient,
		listPodExecSessions,
		stopPodExecSession,
	} from "@/lib/tauri";
	import { queryKeys } from "@/lib/queryKeys";
	import type { PodExecSessionSummary, PortForwardSessionSummary } from "@/lib/types";

	let { onOpenManager }: { onOpenManager: () => void } = $props();

	const client = createTauriClient();
	const queryClient = useQueryClient();
	let popoverOpen = $state(false);
	let stoppingId = $state<string | null>(null);
	let reconnectingId = $state<string | null>(null);
	let copyingId = $state<string | null>(null);
	let actionError = $state<unknown>(null);
	const portForwardsQuery = createQuery(() => portForwardQueryOptions(client));
	const execSessionsQuery = createQuery(() => ({
		queryKey: queryKeys.podExecSessions(),
		queryFn: () => listPodExecSessions(client),
		placeholderData: (previousData) => previousData,
		refetchInterval: 3_000,
	}));

	const portForwardSessions = $derived(portForwardsQuery.data ?? []);
	const execSessions = $derived(sortPodExecSessions(execSessionsQuery.data ?? []));
	const portForwardCount = $derived(portForwardSessions.length);
	const execSessionCount = $derived(execSessions.length);
	const sessionCount = $derived(portForwardCount + execSessionCount);
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

	async function stopPortForward(sessionId: string): Promise<void> {
		stoppingId = sessionId;
		actionError = null;
		try {
			await stopPortForwardLifecycle({
				client,
				sessionId,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
		} catch (error) {
			actionError = error;
		} finally {
			stoppingId = null;
		}
	}

	async function stopExecSession(sessionId: string): Promise<void> {
		stoppingId = sessionId;
		actionError = null;
		try {
			await stopPodExecSession(client, sessionId);
			await queryClient.invalidateQueries({ queryKey: queryKeys.podExecSessions() });
		} catch (error) {
			actionError = error;
		} finally {
			stoppingId = null;
		}
	}

	async function reconnectPortForward(session: PortForwardSessionSummary): Promise<void> {
		reconnectingId = session.id;
		actionError = null;
		try {
			await reconnectPortForwardLifecycle({
				client,
				session,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
		} catch (error) {
			actionError = error;
		} finally {
			reconnectingId = null;
		}
	}

	async function copySessionUrl(session: PortForwardSessionSummary): Promise<void> {
		copyingId = session.id;
		actionError = null;
		try {
			await navigator.clipboard?.writeText(portForwardLocalUrl(session));
		} catch (error) {
			actionError = error;
		} finally {
			copyingId = null;
		}
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
				{#if actionError}
					<FriendlyError
						mode="compact"
						error={actionError}
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
								<Badge variant={session.status === "error" ? "destructive" : "outline"}>
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
								<Button
									type="button"
									variant="outline"
									size="sm"
									disabled={stoppingId === session.id}
									onclick={() => stopExecSession(session.id)}
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
								<Badge variant={session.status === "error" ? "destructive" : "outline"}>
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
									disabled={reconnectingId === session.id}
									onclick={() => reconnectPortForward(session)}
								>
									{#if reconnectingId === session.id}
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
									disabled={copyingId === session.id}
									onclick={() => copySessionUrl(session)}
								>
									<Copy data-icon="inline-start" />
									Copy
								</Button>
								<Button
									type="button"
									variant="outline"
									size="sm"
									disabled={stoppingId === session.id}
									onclick={() => stopPortForward(session.id)}
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
					{#if sessionCount === 0}
						<div class="text-sm text-muted-foreground">Loading active sessions...</div>
					{/if}
				</div>
			</div>
		</PopoverContent>
	</Popover>
{/if}
