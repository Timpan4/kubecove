<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { workspaceStore } from "@/features/workspaces";
	import { getSettingsSnapshot, settingsStore } from "@/lib/settings-store";
	import { createTauriClient } from "@/lib/tauri";
	import type {
		PodExecSessionSummary,
		PortForwardSessionSummary,
	} from "@/lib/types";
	import type {
		SavedPortForward,
		SavedWorkspace,
	} from "@/lib/workspace-model";
	import {
		portForwardErrorMessage,
		portForwardLocalUrl,
		savedPortForwardLabel,
	} from "./helpers";
	import { buildLiveSessionReadModel } from "./liveSessionReadModel";
	import { type SavedPortForwardFormValues } from "./portForwardForms";
	import { podExecQueryOptions, stopPodExec } from "./podExecLifecycle";
	import {
		parseSavedPortForwardForWorkspace,
		portForwardQueryOptions,
		reconnectPortForward as reconnectPortForwardLifecycle,
		startSavedPortForward as startSavedPortForwardLifecycle,
		startSavedPortForwards as startSavedPortForwardsLifecycle,
		stopPortForward as stopPortForwardLifecycle,
	} from "./portForwardLifecycle";
	import LiveSessionsView from "./LiveSessionsView.svelte";

	let {
		workspace,
		sourceReady,
		kubeconfigSourceKey,
		showKubeconfigSourceLabels,
	}: {
		workspace: SavedWorkspace;
		sourceReady: boolean;
		kubeconfigSourceKey?: string;
		showKubeconfigSourceLabels: boolean;
	} = $props();

	const client = createTauriClient();
	const queryClient = useQueryClient();
	let stoppingSessionId = $state<string | null>(null);
	let copyingSessionId = $state<string | null>(null);
	let reconnectingSessionId = $state<string | null>(null);
	let startingSavedPortForwardId = $state<string | null>(null);
	let startingSavedPortForwards = $state(false);
	let savedPortForwardActionMessage = $state<string | null>(null);
	let savedPortForwardFormOpen = $state(false);
	let editingSavedPortForwardId = $state<string | null>(null);
	let savedPortForwardFormError = $state<string | null>(null);
	let savedPortForwardForm = $state<SavedPortForwardFormValues>(newSavedPortForwardForm());
	let liveSessionActionError = $state<unknown>(null);
	const autoStartSavedPortForwards = $derived($settingsStore.autoStartSavedPortForwards);

	const portForwardsQuery = createQuery<PortForwardSessionSummary[]>(() =>
		portForwardQueryOptions(client, {
			enabled: sourceReady,
			refetchInterval: 2_500,
		}),
	);
	const execSessionsQuery = createQuery<PodExecSessionSummary[]>(() =>
		podExecQueryOptions(client, {
			enabled: sourceReady,
			refetchInterval: 2_500,
		}),
	);
	const liveSessionsQuery = $derived({
		isPending: portForwardsQuery.isPending || execSessionsQuery.isPending,
		isError: portForwardsQuery.isError || execSessionsQuery.isError,
		error: portForwardsQuery.error ?? execSessionsQuery.error,
	});
	const liveSessionReadModel = $derived(
		buildLiveSessionReadModel(
			portForwardsQuery.data ?? [],
			execSessionsQuery.data ?? [],
			{ workspace, kubeconfigSource: kubeconfigSourceKey },
		),
	);
	const visiblePortForwardSessions = $derived(liveSessionReadModel.portForwards);
	const visibleExecSessions = $derived(liveSessionReadModel.podExecSessions);

	function portForwardSessionTitle(session: PortForwardSessionSummary): string {
		return `${session.targetKind}/${session.targetName}:${session.remotePort}`;
	}

	function portForwardSessionResolution(session: PortForwardSessionSummary): string {
		if (session.targetKind === "Service") {
			return `Resolved Pod: ${session.resolvedPodName}:${session.resolvedPodPort}`;
		}
		return `Pod: ${session.resolvedPodName}:${session.resolvedPodPort}`;
	}

	function setAutoStartSavedPortForwards(autoStart: boolean) {
		getSettingsSnapshot().setAutoStartSavedPortForwards(autoStart);
	}

	function newSavedPortForwardForm(): SavedPortForwardFormValues {
		return {
			clusterContext: workspace.scope.clusterContext,
			namespace: workspace.scope.namespaces[0] ?? "",
			serviceName: "",
			servicePort: "",
			localPort: "",
			label: "",
		};
	}

	function savedPortForwardFormFromSaved(
		portForward: SavedPortForward,
	): SavedPortForwardFormValues {
		return {
			clusterContext: portForward.clusterContext,
			namespace: portForward.namespace,
			serviceName: portForward.serviceName,
			servicePort: String(portForward.servicePort),
			localPort: portForward.localPort === undefined ? "" : String(portForward.localPort),
			label: portForward.label ?? "",
		};
	}

	function resetSavedPortForwardForm() {
		savedPortForwardFormOpen = false;
		editingSavedPortForwardId = null;
		savedPortForwardFormError = null;
		savedPortForwardForm = newSavedPortForwardForm();
	}

	function beginAddSavedPortForward() {
		editingSavedPortForwardId = null;
		savedPortForwardFormError = null;
		savedPortForwardForm = newSavedPortForwardForm();
		savedPortForwardFormOpen = true;
	}

	function beginEditSavedPortForward(portForward: SavedPortForward) {
		editingSavedPortForwardId = portForward.id;
		savedPortForwardFormError = null;
		savedPortForwardForm = savedPortForwardFormFromSaved(portForward);
		savedPortForwardFormOpen = true;
	}

	function setSavedPortForwardFormValue(
		key: keyof SavedPortForwardFormValues,
		value: string,
	) {
		savedPortForwardForm = { ...savedPortForwardForm, [key]: value };
	}

	function submitSavedPortForwardForm() {
		const parsed = parseSavedPortForwardForWorkspace(savedPortForwardForm, workspace);
		if (typeof parsed === "string") {
			savedPortForwardFormError = parsed;
			return;
		}
		if (editingSavedPortForwardId) {
			workspaceStore.updateSavedPortForward(
				workspace.id,
				editingSavedPortForwardId,
				parsed,
			);
		} else {
			workspaceStore.saveSavedPortForward(workspace.id, parsed);
		}
		savedPortForwardActionMessage = editingSavedPortForwardId
			? "Saved forward updated."
			: "Saved forward added.";
		resetSavedPortForwardForm();
	}

	function deleteSavedPortForward(portForward: SavedPortForward) {
		workspaceStore.deleteSavedPortForward(workspace.id, portForward.id);
		if (editingSavedPortForwardId === portForward.id) resetSavedPortForwardForm();
		savedPortForwardActionMessage = `Deleted ${savedPortForwardLabel(portForward)}.`;
	}

	async function copyPortForwardUrl(session: PortForwardSessionSummary) {
		copyingSessionId = session.id;
		liveSessionActionError = null;
		try {
			await navigator.clipboard.writeText(portForwardLocalUrl(session));
		} catch (error) {
			liveSessionActionError = error;
		} finally {
			copyingSessionId = null;
		}
	}

	async function reconnectPortForward(session: PortForwardSessionSummary) {
		reconnectingSessionId = session.id;
		liveSessionActionError = null;
		savedPortForwardActionMessage = null;
		try {
			await reconnectPortForwardLifecycle({
				client,
				session,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
			savedPortForwardActionMessage = `Reconnected ${portForwardSessionTitle(session)}.`;
		} catch (error) {
			liveSessionActionError = error;
		} finally {
			reconnectingSessionId = null;
		}
	}

	async function startSavedPortForward(portForward: SavedPortForward) {
		startingSavedPortForwardId = portForward.id;
		liveSessionActionError = null;
		savedPortForwardActionMessage = null;
		try {
			const result = await startSavedPortForwardLifecycle({
				client,
				workspaceId: workspace.id,
				portForward,
				knownSessions: portForwardsQuery.data,
				kubeconfigSource: kubeconfigSourceKey,
				updateSavedPortForward: workspaceStore.updateSavedPortForward,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
			if (result.ok && result.skipped) {
				savedPortForwardActionMessage = `${savedPortForwardLabel(portForward)} already active.`;
				return;
			}
			if (!result.ok) {
				liveSessionActionError = result.error ?? "Saved port-forward failed to start.";
				return;
			}
			savedPortForwardActionMessage = `Started ${savedPortForwardLabel(portForward)}.`;
		} catch (error) {
			const message = portForwardErrorMessage(error);
			workspaceStore.updateSavedPortForward(workspace.id, portForward.id, {
				lastStatus: "error",
				lastError: message,
			});
			liveSessionActionError = error;
		} finally {
			startingSavedPortForwardId = null;
		}
	}

	async function startAllSavedPortForwards() {
		startingSavedPortForwards = true;
		liveSessionActionError = null;
		savedPortForwardActionMessage = null;
		try {
			const results = await startSavedPortForwardsLifecycle({
				client,
				workspace,
				portForwards: workspace.portForwards ?? [],
				activeSessions: portForwardsQuery.data,
				kubeconfigSource: kubeconfigSourceKey,
				updateSavedPortForward: workspaceStore.updateSavedPortForward,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
			const failures = results.filter((result) => !result.ok).length;
			const conflicts = results.filter((result) => result.conflict).length;
			savedPortForwardActionMessage =
				failures > 0
					? conflicts > 0
						? `${conflicts} saved ${conflicts === 1 ? "forward has" : "forwards have"} local port conflicts.`
						: `${failures} saved ${failures === 1 ? "forward" : "forwards"} failed to start.`
					: `Started ${results.length} saved ${results.length === 1 ? "forward" : "forwards"}.`;
		} catch (error) {
			liveSessionActionError = error;
		} finally {
			startingSavedPortForwards = false;
		}
	}

	async function stopPortForwardSession(sessionId: string) {
		stoppingSessionId = sessionId;
		liveSessionActionError = null;
		try {
			await stopPortForwardLifecycle({
				client,
				sessionId,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
		} catch (error) {
			liveSessionActionError = error;
		} finally {
			stoppingSessionId = null;
		}
	}

	async function stopExecSession(sessionId: string) {
		stoppingSessionId = sessionId;
		liveSessionActionError = null;
		try {
			await stopPodExec({
				client,
				sessionId,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
		} catch (error) {
			liveSessionActionError = error;
		} finally {
			stoppingSessionId = null;
		}
	}
</script>

<LiveSessionsView
	status={{
		query: liveSessionsQuery,
		actionError: liveSessionActionError,
		actionMessage: savedPortForwardActionMessage,
		showKubeconfigSourceLabels,
	}}
	portForwards={{
		sessions: visiblePortForwardSessions,
		reconnectingId: reconnectingSessionId,
		copyingId: copyingSessionId,
		stoppingId: stoppingSessionId,
		title: portForwardSessionTitle,
		resolution: portForwardSessionResolution,
		copyUrl: copyPortForwardUrl,
		reconnect: reconnectPortForward,
		stop: stopPortForwardSession,
	}}
	savedForwards={{
		workspace,
		autoStart: autoStartSavedPortForwards,
		setAutoStart: setAutoStartSavedPortForwards,
		startingAll: startingSavedPortForwards,
		startAll: startAllSavedPortForwards,
		startingId: startingSavedPortForwardId,
		formOpen: savedPortForwardFormOpen,
		form: savedPortForwardForm,
		formError: savedPortForwardFormError,
		editingId: editingSavedPortForwardId,
		kubeconfigSourceKey,
		beginAdd: beginAddSavedPortForward,
		beginEdit: beginEditSavedPortForward,
		resetForm: resetSavedPortForwardForm,
		submitForm: submitSavedPortForwardForm,
		delete: deleteSavedPortForward,
		start: startSavedPortForward,
		setFormValue: setSavedPortForwardFormValue,
	}}
	podExec={{
		sessions: visibleExecSessions,
		stop: stopExecSession,
	}}
/>
