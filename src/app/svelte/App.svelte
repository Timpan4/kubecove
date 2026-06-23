	<script lang="ts">
	import { onMount } from "svelte";
	import { useQueryClient } from "@tanstack/svelte-query";
	import { FolderOpen } from "lucide-svelte";
	import RuntimeBadge from "../runtime/RuntimeBadge.svelte";
	import { Button } from "@/components/ui/svelte";
	import AppUsageFooter from "./AppUsageFooter.svelte";
	import SettingsSurface from "./SettingsSurface.svelte";
	import UpdateStatusButton from "./UpdateStatusButton.svelte";
	import WorkspaceShell from "./WorkspaceShell.svelte";
	import WorkspaceLauncher from "@/features/workspaces/WorkspaceLauncher.svelte";
	import { workspaceStore } from "@/features/workspaces/workspaceStore";
	import { diagnosticLog, setDiagnosticsEnabled } from "@/lib/diagnostics";
	import { queryKeys } from "@/lib/queryKeys";
	import { isAppUpdatesEnabled } from "@/lib/release-channel";
	import {
		createTauriClient,
		setBackendDiagnosticsEnabled,
		stopLiveSessionsOutsideScope,
	} from "@/lib/tauri";
	import {
		takeUiRuntimeReloadNotice,
		takeUiRuntimeSettingsOpen,
		takeUiRuntimeWorkspaceHandoff,
		type UiRuntimeWorkspaceHandoff,
	} from "@/lib/ui-runtime";
	import {
		readPathState,
		writePathState,
		type PathStateWorkspaceSnapshot,
	} from "@/lib/path-state";
	import { settingsStore } from "@/lib/settings-store";
	import { workspaceScopeContexts } from "@/lib/workspace-model";
	import { appUpdateActions } from "./appUpdateStore";

	const selectedWorkspace = workspaceStore.selectedWorkspace;
	const diagnosticsClient = createTauriClient();
	const liveSessionClient = createTauriClient();
	const queryClient = useQueryClient();
	let notice = $state("");
	let liveSessionCleanupMessage = $state<string | null>(null);
	let openSettingsOnWorkspaceMount = $state(false);
	let runtimeWorkspaceHandoff = $state<UiRuntimeWorkspaceHandoff | null>(null);
	let initialWorkspacePathState = $state<PathStateWorkspaceSnapshot | null>(null);
	let launcherView = $state<"workspaces" | "settings">("workspaces");
	let pathStateReady = $state(false);
	let lastDiagnosticsEnabled: boolean | null = null;
	let liveSessionScopeInitialized = false;
	let lastLiveSessionCleanupKey = "";

	function openLauncherSettings() {
		launcherView = "settings";
	}

	function openWorkspaceLauncher() {
		launcherView = "workspaces";
	}

	onMount(() => {
		const message = takeUiRuntimeReloadNotice();
		if (!message) return;
		notice = message;
		const timeout = window.setTimeout(() => {
			notice = "";
		}, 3_000);
		return () => window.clearTimeout(timeout);
	});

	onMount(() => {
		const handoff = takeUiRuntimeWorkspaceHandoff();
		if (handoff) {
			runtimeWorkspaceHandoff = handoff;
			workspaceStore.openWorkspace(handoff.workspaceId);
		} else {
			const pathState = readPathState();
			if (pathState?.workspace) {
				initialWorkspacePathState = pathState.workspace;
				workspaceStore.openWorkspace(pathState.workspace.workspaceId);
			} else if (pathState?.launcherView) {
				launcherView = pathState.launcherView;
			}
		}
		openSettingsOnWorkspaceMount = takeUiRuntimeSettingsOpen();
		if (openSettingsOnWorkspaceMount && !$selectedWorkspace) launcherView = "settings";
		pathStateReady = true;
	});

	onMount(() => {
		if (!isAppUpdatesEnabled()) return;
		void appUpdateActions.checkForUpdates({ manual: false });
	});

	function syncDiagnostics(enabled: boolean) {
		setDiagnosticsEnabled(enabled);
		void setBackendDiagnosticsEnabled(diagnosticsClient, enabled).catch((error) => {
			diagnosticLog("diagnostics.backend.toggle.error", {
				error: error instanceof Error ? error.message : String(error),
			});
		});
	}

	$effect(() => {
		const enabled = $settingsStore.debugModeEnabled;
		if (lastDiagnosticsEnabled === enabled) return;
		lastDiagnosticsEnabled = enabled;
		syncDiagnostics(enabled);
	});

	$effect(() => {
		if (!pathStateReady || $selectedWorkspace) return;
		writePathState({
			version: 1,
			runtime: "svelte",
			launcherView,
			workspace: null,
		});
	});

	$effect(() => {
		const workspace = $selectedWorkspace;
		const allowedClusterContexts = workspace
			? workspaceScopeContexts(workspace.scope)
			: [];
		const keepLiveSessions =
			$settingsStore.keepLiveSessionsOnWorkspaceSwitch;
		const kubeconfigSourceKey = $settingsStore.kubeconfigSourceKey;
		const cleanupKey = JSON.stringify({
			workspaceId: workspace?.id ?? null,
			allowedClusterContexts,
			keepLiveSessions,
			kubeconfigSourceKey,
		});
		if (lastLiveSessionCleanupKey === cleanupKey) return;
		lastLiveSessionCleanupKey = cleanupKey;

		if (!liveSessionScopeInitialized) {
			liveSessionScopeInitialized = true;
			return;
		}
		if (keepLiveSessions) return;

		let cancelled = false;
		void stopLiveSessionsOutsideScope(liveSessionClient, {
			allowedClusterContexts,
			kubeconfigSourceKey,
		})
			.then(async (result) => {
				if (cancelled) return;
				await Promise.all([
					queryClient.invalidateQueries({ queryKey: queryKeys.portForwards() }),
					queryClient.invalidateQueries({
						queryKey: queryKeys.podExecSessions(),
					}),
				]);
				const stopped =
					result.stoppedPortForwards + result.stoppedPodExecSessions;
				if (stopped > 0) {
					liveSessionCleanupMessage = `Stopped ${result.stoppedPortForwards} port ${result.stoppedPortForwards === 1 ? "forward" : "forwards"} and ${result.stoppedPodExecSessions} exec ${result.stoppedPodExecSessions === 1 ? "session" : "sessions"} outside this workspace.`;
				}
			})
			.catch((error: unknown) => {
				if (cancelled) return;
				liveSessionCleanupMessage = `Live session cleanup failed: ${error instanceof Error ? error.message : String(error)}`;
			});

		return () => {
			cancelled = true;
		};
	});
</script>

<svelte:head>
	<title>KubeCove Svelte UI</title>
</svelte:head>

{#if $selectedWorkspace}
	<WorkspaceShell
		workspace={$selectedWorkspace}
		{openSettingsOnWorkspaceMount}
		{runtimeWorkspaceHandoff}
		initialPathState={initialWorkspacePathState?.workspaceId === $selectedWorkspace.id
			? initialWorkspacePathState
			: null}
		onRuntimeWorkspaceHandoffConsumed={() => (runtimeWorkspaceHandoff = null)}
		onPathStateConsumed={() => (initialWorkspacePathState = null)}
		{liveSessionCleanupMessage}
		onDismissLiveSessionCleanup={() => (liveSessionCleanupMessage = null)}
	/>
{:else}
	<div class="flex h-screen w-full flex-col overflow-hidden bg-background text-foreground">
		<header class="flex h-12 shrink-0 items-center gap-4 border-b bg-sidebar px-4 [-webkit-app-region:drag]">
			<div class="flex min-w-0 flex-1 items-center gap-2 [-webkit-app-region:no-drag]">
				<span class="truncate whitespace-nowrap font-heading text-sm font-semibold">KubeCove</span>
				<span class="hidden text-xs text-muted-foreground sm:inline">
					{launcherView === "settings" ? "Settings" : "Workspaces"}
				</span>
			</div>
			<div class="flex min-w-0 flex-1 items-center justify-center">
				<span class="truncate whitespace-nowrap text-sm font-semibold">
					{launcherView === "settings" ? "Settings" : "Workspaces"}
				</span>
			</div>
			<div class="flex flex-1 items-center justify-end gap-1 [-webkit-app-region:no-drag]">
				<UpdateStatusButton />
				<RuntimeBadge mode="svelte" onOpenSettings={openLauncherSettings} />
				{#if launcherView === "settings"}
					<Button
						type="button"
						variant="ghost"
						size="icon"
						aria-label="Open workspaces"
						onclick={openWorkspaceLauncher}
					>
						<FolderOpen />
					</Button>
				{/if}
			</div>
		</header>

		<main class="min-h-0 flex-1 overflow-y-auto">
			{#if launcherView === "settings"}
				<section class="mx-auto flex w-full max-w-6xl flex-col gap-4 p-4 md:p-6">
					<SettingsSurface />
				</section>
			{:else}
				<WorkspaceLauncher />
			{/if}
		</main>
		<AppUsageFooter />
	</div>
{/if}

{#if notice}
	<div
		role="status"
		class="fixed bottom-4 left-1/2 z-toast -translate-x-1/2 rounded-md border border-border/60 bg-surface-2 px-3 py-2 text-sm text-popover-foreground shadow-xl backdrop-blur-2xl"
	>
		{notice}
	</div>
{/if}
