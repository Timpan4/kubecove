<script lang="ts">
	import { Copy, Link2, Pencil, Play, Plus, RotateCcw, Square, Trash2 } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Badge,
		Button,
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle,
		Checkbox,
		Field,
		FieldLabel,
		Spinner,
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow,
	} from "@/components/ui/svelte";
	import {
		portForwardLocalUrl,
		savedPortForwardLabel,
		savedPortForwardMatchesSession,
	} from "@/features/live-sessions/helpers";
	import { podExecCommandText } from "./podExecHelpers";
	import type { PodExecSessionSummary, PortForwardSessionSummary } from "@/lib/types";
	import type { SavedPortForward, SavedWorkspace } from "@/lib/workspace-model";
	import StatGrid from "@/components/StatGrid.svelte";
	import SurfaceFrame from "@/components/SurfaceFrame.svelte";
	import SavedPortForwardForm from "./SavedPortForwardForm.svelte";
	import type { SavedPortForwardFormValues } from "./portForwardForms";

	type LiveSessionStatus = {
		query: { isPending: boolean; isError: boolean; error: unknown };
		actionError: unknown;
		actionMessage: string | null;
		showKubeconfigSourceLabels: boolean;
	};

	type PortForwardViewModel = {
		sessions: PortForwardSessionSummary[];
		reconnectingId: string | null;
		copyingId: string | null;
		stoppingId: string | null;
		title: (session: PortForwardSessionSummary) => string;
		resolution: (session: PortForwardSessionSummary) => string;
		copyUrl: (session: PortForwardSessionSummary) => Promise<void>;
		reconnect: (session: PortForwardSessionSummary) => Promise<void>;
		stop: (sessionId: string) => Promise<void>;
	};

	type SavedForwardViewModel = {
		workspace: SavedWorkspace;
		autoStart: boolean;
		setAutoStart: (autoStart: boolean) => void;
		startingAll: boolean;
		startAll: () => Promise<void>;
		startingId: string | null;
		formOpen: boolean;
		form: SavedPortForwardFormValues;
		formError: string | null;
		editingId: string | null;
		kubeconfigSourceKey?: string;
		beginAdd: () => void;
		beginEdit: (portForward: SavedPortForward) => void;
		resetForm: () => void;
		submitForm: () => void;
		delete: (portForward: SavedPortForward) => void;
		start: (portForward: SavedPortForward) => Promise<void>;
		setFormValue: (key: keyof SavedPortForwardFormValues, value: string) => void;
	};

	type PodExecViewModel = {
		sessions: PodExecSessionSummary[];
		stop: (sessionId: string) => Promise<void>;
	};

	let {
		status,
		portForwards,
		savedForwards,
		podExec,
	}: {
		status: LiveSessionStatus;
		portForwards: PortForwardViewModel;
		savedForwards: SavedForwardViewModel;
		podExec: PodExecViewModel;
	} = $props();
	const liveSessionsQuery = $derived(status.query);
	const liveSessionActionError = $derived(status.actionError);
	const savedPortForwardActionMessage = $derived(status.actionMessage);
	const showKubeconfigSourceLabels = $derived(status.showKubeconfigSourceLabels);
	const visiblePortForwardSessions = $derived(portForwards.sessions);
	const reconnectingSessionId = $derived(portForwards.reconnectingId);
	const copyingSessionId = $derived(portForwards.copyingId);
	const stoppingSessionId = $derived(portForwards.stoppingId);
	const portForwardSessionTitle = $derived(portForwards.title);
	const portForwardSessionResolution = $derived(portForwards.resolution);
	const copyPortForwardUrl = $derived(portForwards.copyUrl);
	const reconnectPortForward = $derived(portForwards.reconnect);
	const stopPortForwardSession = $derived(portForwards.stop);
	const workspace = $derived(savedForwards.workspace);
	const autoStartSavedPortForwards = $derived(savedForwards.autoStart);
	const setAutoStartSavedPortForwards = $derived(savedForwards.setAutoStart);
	const startingSavedPortForwards = $derived(savedForwards.startingAll);
	const startAllSavedPortForwards = $derived(savedForwards.startAll);
	const startingSavedPortForwardId = $derived(savedForwards.startingId);
	const savedPortForwardFormOpen = $derived(savedForwards.formOpen);
	const savedPortForwardForm = $derived(savedForwards.form);
	const savedPortForwardFormError = $derived(savedForwards.formError);
	const editingSavedPortForwardId = $derived(savedForwards.editingId);
	const kubeconfigSourceKey = $derived(savedForwards.kubeconfigSourceKey);
	const beginAddSavedPortForward = $derived(savedForwards.beginAdd);
	const beginEditSavedPortForward = $derived(savedForwards.beginEdit);
	const resetSavedPortForwardForm = $derived(savedForwards.resetForm);
	const submitSavedPortForwardForm = $derived(savedForwards.submitForm);
	const deleteSavedPortForward = $derived(savedForwards.delete);
	const startSavedPortForward = $derived(savedForwards.start);
	const setSavedPortForwardFormValue = $derived(savedForwards.setFormValue);
	const visibleExecSessions = $derived(podExec.sessions);
	const stopExecSession = $derived(podExec.stop);
</script>

<SurfaceFrame icon={Link2} title="Live Sessions" query={liveSessionsQuery} errorLabel="Live sessions unavailable">
		<div class="flex flex-wrap items-center justify-between gap-2 rounded-md border bg-background/50 px-3 py-2">
			<Field orientation="horizontal" class="w-auto">
				<FieldLabel class="items-center gap-2 text-xs text-muted-foreground">
					<Checkbox
						checked={autoStartSavedPortForwards}
						aria-label="Auto-start saved port forwards"
						onCheckedChange={setAutoStartSavedPortForwards}
					/>
					Auto-start saved
				</FieldLabel>
			</Field>
			<Button
				type="button"
				variant="outline"
				size="sm"
				disabled={startingSavedPortForwards || (workspace.portForwards ?? []).length === 0}
				onclick={startAllSavedPortForwards}
			>
				{#if startingSavedPortForwards}
					<Spinner data-icon="inline-start" />
				{:else}
					<Play data-icon="inline-start" />
				{/if}
				Start saved
			</Button>
		</div>
		<StatGrid
			stats={[
				["Port forwards", visiblePortForwardSessions.length],
				["Exec sessions", visibleExecSessions.length],
			]}
		/>
		{#if liveSessionActionError}
			<FriendlyError
				error={liveSessionActionError}
				context={{ operation: "liveSession", fallbackTitle: "Live-session action failed" }}
			/>
		{/if}
		{#if savedPortForwardActionMessage}
			<Alert>
				<AlertTitle>Saved forward</AlertTitle>
				<AlertDescription>{savedPortForwardActionMessage}</AlertDescription>
			</Alert>
		{/if}
		<Card size="sm" elevation="flat">
			<CardHeader>
				<CardTitle>Port forwards</CardTitle>
				<CardDescription>Active local tunnels with guarded stop controls.</CardDescription>
			</CardHeader>
			<CardContent class="p-0">
				<Table class="min-w-[980px] table-fixed text-sm">
					<TableHeader>
						<TableRow>
							{#each ["Target", "Namespace", "Local", "Resolved", "Status", "Actions"] as header}
								<TableHead class="px-3 py-2 text-xs font-semibold uppercase text-muted-foreground">
									{header}
								</TableHead>
							{/each}
						</TableRow>
					</TableHeader>
					<TableBody>
						{#if visiblePortForwardSessions.length === 0}
							<TableRow>
								<TableCell class="px-3 py-8 text-center text-muted-foreground" colspan="6">No active port forwards</TableCell>
							</TableRow>
						{:else}
							{#each visiblePortForwardSessions as session}
								<TableRow>
									<TableCell class="truncate px-3 py-2">
										<div class="truncate font-medium">{portForwardSessionTitle(session)}</div>
										{#if showKubeconfigSourceLabels && session.kubeconfigSourceLabel}
											<div class="truncate text-xs text-muted-foreground">{session.kubeconfigSourceLabel}</div>
										{/if}
									</TableCell>
									<TableCell class="truncate px-3 py-2">{session.namespace}</TableCell>
									<TableCell class="truncate px-3 py-2">{portForwardLocalUrl(session)}</TableCell>
									<TableCell class="truncate px-3 py-2">{portForwardSessionResolution(session)}</TableCell>
									<TableCell class="truncate px-3 py-2">
										<Badge variant={session.status === "error" ? "destructive" : "outline"}>{session.status}</Badge>
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
									</TableCell>
									<TableCell class="px-3 py-2">
										<div class="flex flex-wrap gap-1">
											<Button
												type="button"
												variant="outline"
												size="xs"
												disabled={reconnectingSessionId === session.id}
												onclick={() => reconnectPortForward(session)}
											>
												<RotateCcw data-icon="inline-start" />
												Reconnect
											</Button>
											<Button
												type="button"
												variant="outline"
												size="xs"
												disabled={copyingSessionId === session.id}
												onclick={() => copyPortForwardUrl(session)}
											>
												<Copy data-icon="inline-start" />
												Copy
											</Button>
											<Button
												type="button"
												variant="destructive"
												size="xs"
												disabled={stoppingSessionId === session.id}
												onclick={() => stopPortForwardSession(session.id)}
											>
												<Square data-icon="inline-start" />
												Stop
											</Button>
										</div>
									</TableCell>
								</TableRow>
							{/each}
						{/if}
					</TableBody>
				</Table>
			</CardContent>
		</Card>
		<Card size="sm" elevation="flat">
			<CardHeader class="flex flex-row items-start justify-between gap-3">
				<div class="min-w-0">
					<CardTitle>Saved Service forwards</CardTitle>
					<CardDescription>Launch saved selector-backed Service forwards for this workspace.</CardDescription>
				</div>
				<Button type="button" variant="outline" size="sm" onclick={beginAddSavedPortForward}>
					<Plus data-icon="inline-start" />
					Save forward
				</Button>
			</CardHeader>
			<CardContent class="space-y-3">
				{#if savedPortForwardFormOpen}
					<SavedPortForwardForm
						form={savedPortForwardForm}
						error={savedPortForwardFormError}
						editingId={editingSavedPortForwardId}
						onValue={setSavedPortForwardFormValue}
						onCancel={resetSavedPortForwardForm}
						onSubmit={submitSavedPortForwardForm}
					/>
				{/if}
				<div>
					<Table class="min-w-[980px] table-fixed text-sm">
						<TableHeader>
							<TableRow>
								{#each ["Preset", "Cluster", "Namespace", "Service", "Local", "Status", "Actions"] as header}
									<TableHead class="px-3 py-2 text-xs font-semibold uppercase text-muted-foreground">
										{header}
									</TableHead>
								{/each}
							</TableRow>
						</TableHeader>
						<TableBody>
							{#if (workspace.portForwards ?? []).length === 0}
								<TableRow>
									<TableCell class="px-3 py-8 text-center text-muted-foreground" colspan="7">No saved Service forwards</TableCell>
								</TableRow>
							{:else}
								{#each workspace.portForwards ?? [] as portForward}
				{@const activeSession = visiblePortForwardSessions.find((session: PortForwardSessionSummary) =>
									savedPortForwardMatchesSession(portForward, session, kubeconfigSourceKey)
								)}
									<TableRow>
										<TableCell class="truncate px-3 py-2 font-medium">{savedPortForwardLabel(portForward)}</TableCell>
										<TableCell class="truncate px-3 py-2">{portForward.clusterContext}</TableCell>
										<TableCell class="truncate px-3 py-2">{portForward.namespace}</TableCell>
										<TableCell class="truncate px-3 py-2">Service/{portForward.serviceName}:{portForward.servicePort}</TableCell>
										<TableCell class="truncate px-3 py-2">{activeSession ? portForwardLocalUrl(activeSession) : (portForward.localPort ?? "Auto")}</TableCell>
										<TableCell class="truncate px-3 py-2">
											<div class="flex flex-wrap gap-1">
												{#if activeSession}
													<Badge variant="secondary">active</Badge>
												{/if}
												<Badge variant={portForward.lastStatus === "error" ? "destructive" : "outline"}>
													{portForward.lastStatus ?? "idle"}
												</Badge>
									</div>
									{#if portForward.lastError}
										<FriendlyError
											mode="compact"
											error={portForward.lastError}
											context={{
												operation: "portForward",
												fallbackTitle: "Port-forward failed",
												partial: true,
											}}
										/>
									{/if}
										</TableCell>
										<TableCell class="px-3 py-2">
											<div class="flex flex-wrap gap-1">
												<Button
													type="button"
													variant="outline"
													size="xs"
													disabled={startingSavedPortForwardId === portForward.id}
													onclick={() => startSavedPortForward(portForward)}
												>
													<Play data-icon="inline-start" />
													Start
												</Button>
												<Button type="button" variant="outline" size="xs" onclick={() => beginEditSavedPortForward(portForward)}>
													<Pencil data-icon="inline-start" />
													Edit
												</Button>
												<Button type="button" variant="outline" size="xs" onclick={() => deleteSavedPortForward(portForward)}>
													<Trash2 data-icon="inline-start" />
													Delete
												</Button>
											</div>
										</TableCell>
									</TableRow>
								{/each}
							{/if}
						</TableBody>
					</Table>
				</div>
			</CardContent>
		</Card>
		<Card size="sm" elevation="flat">
			<CardHeader>
				<CardTitle>Pod exec sessions</CardTitle>
				<CardDescription>Active interactive Pod sessions with guarded stop controls.</CardDescription>
			</CardHeader>
			<CardContent class="p-0">
				<Table class="min-w-[760px] table-fixed text-sm">
					<TableHeader>
						<TableRow>
							{#each ["Pod", "Namespace", "Command", "Status", "Started", "Actions"] as header}
								<TableHead class="px-3 py-2 text-xs font-semibold uppercase text-muted-foreground">
									{header}
								</TableHead>
							{/each}
						</TableRow>
					</TableHeader>
					<TableBody>
						{#if visibleExecSessions.length === 0}
							<TableRow>
								<TableCell class="px-3 py-8 text-center text-muted-foreground" colspan="6">No active exec sessions</TableCell>
							</TableRow>
						{:else}
							{#each visibleExecSessions as session}
								<TableRow>
								<TableCell class="truncate px-3 py-2">
									<div class="truncate font-medium">{session.podName}</div>
									{#if showKubeconfigSourceLabels && session.kubeconfigSourceLabel}
										<div class="truncate text-xs text-muted-foreground">{session.kubeconfigSourceLabel}</div>
									{/if}
								</TableCell>
								<TableCell class="truncate px-3 py-2">{session.namespace}</TableCell>
								<TableCell class="truncate px-3 py-2">{podExecCommandText(session.command)}</TableCell>
									<TableCell class="truncate px-3 py-2">{session.status}</TableCell>
									<TableCell class="truncate px-3 py-2">{session.startedAt}</TableCell>
									<TableCell class="px-3 py-2">
										<Button
											type="button"
											variant="destructive"
											size="xs"
											disabled={stoppingSessionId === session.id}
											onclick={() => stopExecSession(session.id)}
										>
											<Square data-icon="inline-start" />
											Stop
										</Button>
									</TableCell>
								</TableRow>
							{/each}
						{/if}
					</TableBody>
				</Table>
			</CardContent>
		</Card>
	</SurfaceFrame>
