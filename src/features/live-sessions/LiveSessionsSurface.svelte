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
	import type { SavedPortForwardFormState, SavedPortForwardFormValues } from "./portForwardForms";
	import { LiveSessionRowActions, savedForwardRowId } from "./rowActions.svelte";
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
	import { trackLiveSessionUi } from "./sessionPolling.svelte";

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
	const rows = new LiveSessionRowActions();
	let startingSavedPortForwards = $state(false);
	let savedPortForwardActionMessage = $state<string | null>(null);
	let savedForwardForm = $state<SavedPortForwardFormState>({ mode: "closed" });
	const autoStartSavedPortForwards = $derived($settingsStore.autoStartSavedPortForwards);
	trackLiveSessionUi(() => sourceReady);

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
	const liveSessionsQuery = $derived.by(() => {
		const portForwards = portForwardsQuery.data;
		const execSessions = execSessionsQuery.data;
		return {
			data: portForwards !== undefined || execSessions !== undefined ? { portForwards, execSessions } : undefined,
			isPending: portForwardsQuery.isPending || execSessionsQuery.isPending,
			isError: portForwardsQuery.isError || execSessionsQuery.isError,
			error: portForwardsQuery.error ?? execSessionsQuery.error,
		};
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
			return `Latest Pod used: ${session.resolvedPodName}:${session.resolvedPodPort} (each connection may use a different ready Pod)`;
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
		savedForwardForm = { mode: "closed" };
	}

	function beginAddSavedPortForward() {
		savedForwardForm = { mode: "create", values: newSavedPortForwardForm(), error: null };
	}

	function beginEditSavedPortForward(portForward: SavedPortForward) {
		savedForwardForm = {
			mode: "edit",
			id: portForward.id,
			values: savedPortForwardFormFromSaved(portForward),
			error: null,
		};
	}

	function setSavedPortForwardFormValue(
		key: keyof SavedPortForwardFormValues,
		value: string,
	) {
		if (savedForwardForm.mode === "closed") return;
		savedForwardForm = {
			...savedForwardForm,
			values: { ...savedForwardForm.values, [key]: value },
		};
	}

	function submitSavedPortForwardForm() {
		const form = savedForwardForm;
		if (form.mode === "closed") return;
		const parsed = parseSavedPortForwardForWorkspace(form.values, workspace);
		if (isValidationMessage(parsed)) {
			savedForwardForm = { ...form, error: parsed };
			return;
		}
		if (form.mode === "edit") {
			workspaceStore.updateSavedPortForward(workspace.id, form.id, parsed);
			savedPortForwardActionMessage = "Saved forward updated.";
		} else {
			workspaceStore.saveSavedPortForward(workspace.id, parsed);
			savedPortForwardActionMessage = "Saved forward added.";
		}
		resetSavedPortForwardForm();
	}

	function deleteSavedPortForward(portForward: SavedPortForward) {
		workspaceStore.deleteSavedPortForward(workspace.id, portForward.id);
		if (savedForwardForm.mode === "edit" && savedForwardForm.id === portForward.id) {
			resetSavedPortForwardForm();
		}
		savedPortForwardActionMessage = `Deleted ${savedPortForwardLabel(portForward)}.`;
	}

	function isValidationMessage<Value>(value: Value): value is Extract<Value, string> {
		return Object(value) !== value;
	}

	function copyPortForwardUrl(session: PortForwardSessionSummary): Promise<boolean> {
		return rows.run(session.id, "copy", () =>
			navigator.clipboard.writeText(portForwardLocalUrl(session)),
		);
	}

	async function reconnectPortForward(session: PortForwardSessionSummary): Promise<void> {
		savedPortForwardActionMessage = null;
		const reconnected = await rows.run(session.id, "reconnect", () =>
			reconnectPortForwardLifecycle({
				client,
				session,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			}),
		);
		if (reconnected) {
			savedPortForwardActionMessage = `Reconnected ${portForwardSessionTitle(session)}.`;
		}
	}

	async function startSavedPortForward(portForward: SavedPortForward): Promise<void> {
		savedPortForwardActionMessage = null;
		let message: string | null = null;
		await rows.run(savedForwardRowId(portForward.id), "start", async () => {
			let result: Awaited<ReturnType<typeof startSavedPortForwardLifecycle>>;
			try {
				result = await startSavedPortForwardLifecycle({
					client,
					workspaceId: workspace.id,
					portForward,
					knownSessions: portForwardsQuery.data,
					kubeconfigSource: kubeconfigSourceKey,
					updateSavedPortForward: workspaceStore.updateSavedPortForward,
					invalidateQueries: (options) => queryClient.invalidateQueries(options),
				});
			} catch (error) {
				workspaceStore.updateSavedPortForward(workspace.id, portForward.id, {
					lastStatus: "error",
					lastError: portForwardErrorMessage(error),
				});
				throw error;
			}
			if (!result.ok) throw result.error ?? new Error("Saved port-forward failed to start.");
			message = result.skipped
				? `${savedPortForwardLabel(portForward)} already active.`
				: `Started ${savedPortForwardLabel(portForward)}.`;
		});
		savedPortForwardActionMessage = message;
	}

	async function startAllSavedPortForwards() {
		startingSavedPortForwards = true;
		rows.failure = null;
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
			rows.failure = { action: "start", error };
		} finally {
			startingSavedPortForwards = false;
		}
	}

	function stopPortForwardSession(sessionId: string): Promise<boolean> {
		return rows.run(sessionId, "stop", () =>
			stopPortForwardLifecycle({
				client,
				sessionId,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			}),
		);
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
</script>

<LiveSessionsView
	status={{
		query: liveSessionsQuery,
		actionError: rows.failure?.error ?? null,
		actionMessage: savedPortForwardActionMessage,
		showKubeconfigSourceLabels,
	}}
	{rows}
	portForwards={{
		sessions: visiblePortForwardSessions,
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
		form: savedForwardForm,
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
