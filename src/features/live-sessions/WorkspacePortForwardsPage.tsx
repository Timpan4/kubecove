import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
	Cable,
	Copy,
	Pencil,
	Play,
	Plus,
	RotateCcw,
	Save,
	Square,
	Trash2,
	X,
} from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
import {
	Field,
	FieldDescription,
	FieldGroup,
	FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { ToggleButton } from "@/components/ToggleButton";
import { useSettingsState } from "@/lib/settings";
import {
	createTauriClient,
	listPortForwards,
	stopPodPortForward,
} from "@/lib/tauri";
import type { PortForwardSessionSummary } from "@/lib/types";
import { queryKeys } from "@/lib/queryKeys";
import {
	useWorkspaceStore,
	workspaceScopeContexts,
	type SavePortForwardInput,
	type SavedPortForward,
	type SavedWorkspace,
} from "@/lib/workspaces";
import {
	parseSavedPortForwardForm,
	portForwardErrorMessage,
	portForwardLocalUrl,
	savedPortForwardLabel,
	savedPortForwardMatchesSession,
	sortPortForwardSessions,
	type SavedPortForwardFormValues,
} from "./helpers";
import { useReconnectPortForwardSession } from "./useReconnectPortForwardSession";
import { useSavedPortForwardActions } from "./useSavedPortForwardActions";

interface WorkspacePortForwardsPageProps {
	workspace: SavedWorkspace;
}

const EMPTY_SESSIONS: PortForwardSessionSummary[] = [];

function newFormState(workspace: SavedWorkspace): SavedPortForwardFormValues {
	return {
		clusterContext: workspace.scope.clusterContext,
		namespace: workspace.scope.namespaces[0] ?? "",
		serviceName: "",
		servicePort: "",
		localPort: "",
		label: "",
	};
}

function formStateFromSaved(
	portForward: SavedPortForward,
): SavedPortForwardFormValues {
	return {
		clusterContext: portForward.clusterContext,
		namespace: portForward.namespace,
		serviceName: portForward.serviceName,
		servicePort: String(portForward.servicePort),
		localPort:
			portForward.localPort === undefined ? "" : String(portForward.localPort),
		label: portForward.label ?? "",
	};
}

function activeSessionTitle(session: PortForwardSessionSummary): string {
	return `${session.namespace}/${session.targetKind}/${session.targetName}:${session.remotePort}`;
}

function sessionInWorkspaceScope(
	workspace: SavedWorkspace,
	session: PortForwardSessionSummary,
): boolean {
	return workspaceScopeContexts(workspace.scope).includes(session.clusterContext);
}

function validateSavedPortForwardScope(
	workspace: SavedWorkspace,
	input: SavePortForwardInput,
): string | null {
	if (!workspaceScopeContexts(workspace.scope).includes(input.clusterContext)) {
		return "Cluster context must be in the current workspace scope.";
	}
	return null;
}

async function copyText(text: string): Promise<void> {
	await navigator.clipboard?.writeText(text);
}

export function WorkspacePortForwardsPage({
	workspace,
}: WorkspacePortForwardsPageProps) {
	const client = useMemo(() => createTauriClient(), []);
	const queryClient = useQueryClient();
	const savePortForward = useWorkspaceStore((state) => state.savePortForward);
	const updateSavedPortForward = useWorkspaceStore(
		(state) => state.updateSavedPortForward,
	);
	const deleteSavedPortForward = useWorkspaceStore(
		(state) => state.deleteSavedPortForward,
	);
	const autoStartSavedPortForwards = useSettingsState(
		(state) => state.autoStartSavedPortForwards,
	);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	const showKubeconfigSourceLabels = useSettingsState(
		(state) => state.showKubeconfigSourceLabels,
	);
	const setAutoStartSavedPortForwards = useSettingsState(
		(state) => state.setAutoStartSavedPortForwards,
	);
	const savedPortForwards = workspace.portForwards ?? [];
	const sessionsQuery = useQuery({
		queryKey: queryKeys.portForwards(),
		queryFn: () => listPortForwards(client),
		placeholderData: (previousData) => previousData,
		refetchInterval: 3_000,
	});
	const allSessions = useMemo(
		() =>
			sessionsQuery.data
				? sortPortForwardSessions(sessionsQuery.data)
				: undefined,
		[sessionsQuery.data],
	);
	const sessionsForActions = useMemo(
		() =>
			allSessions
				? sortPortForwardSessions(
						allSessions.filter((session) =>
							sessionInWorkspaceScope(workspace, session),
						),
					)
				: undefined,
		[allSessions, workspace],
	);
	const sessions = sessionsForActions ?? EMPTY_SESSIONS;
	const { startOne, startAll, startingIds, startingAll } =
		useSavedPortForwardActions(workspace, allSessions);
	const [formOpen, setFormOpen] = useState(false);
	const [editingId, setEditingId] = useState<string | null>(null);
	const [form, setForm] = useState<SavedPortForwardFormValues>(() =>
		newFormState(workspace),
	);
	const [formError, setFormError] = useState<string | null>(null);
	const [stoppingId, setStoppingId] = useState<string | null>(null);
	const [copyingId, setCopyingId] = useState<string | null>(null);
	const [bulkMessage, setBulkMessage] = useState<string | null>(null);
	const { reconnectingId, reconnectSession } =
		useReconnectPortForwardSession({
			client,
			onSuccess: (session) =>
				setBulkMessage(`Reconnected ${activeSessionTitle(session)}.`),
			onError: (error) =>
				setBulkMessage(`Reconnect failed: ${portForwardErrorMessage(error)}`),
		});

	const resetForm = () => {
		setEditingId(null);
		setForm(newFormState(workspace));
		setFormError(null);
		setFormOpen(false);
	};

	const beginAdd = () => {
		setEditingId(null);
		setForm(newFormState(workspace));
		setFormError(null);
		setFormOpen(true);
	};

	const beginEdit = (portForward: SavedPortForward) => {
		setEditingId(portForward.id);
		setForm(formStateFromSaved(portForward));
		setFormError(null);
		setFormOpen(true);
	};

	const updateForm = (
		key: keyof SavedPortForwardFormValues,
		value: string,
	) => {
		setForm((current) => ({ ...current, [key]: value }));
	};

	const submitForm = () => {
		const parsed = parseSavedPortForwardForm(form);
		if (typeof parsed === "string") {
			setFormError(parsed);
			return;
		}
		const scopeError = validateSavedPortForwardScope(workspace, parsed);
		if (scopeError) {
			setFormError(scopeError);
			return;
		}
		if (editingId) updateSavedPortForward(workspace.id, editingId, parsed);
		else savePortForward(workspace.id, parsed);
		resetForm();
	};

	const handleStartAll = async () => {
		const results = await startAll(savedPortForwards);
		const failures = results.filter((result) => !result.ok).length;
		const conflicts = results.filter((result) => result.conflict).length;
		setBulkMessage(
			failures > 0
				? conflicts > 0
					? `${conflicts} saved ${conflicts === 1 ? "forward has" : "forwards have"} local port conflicts.`
					: `${failures} saved ${failures === 1 ? "forward" : "forwards"} failed to start.`
				: `Started ${results.length} saved ${results.length === 1 ? "forward" : "forwards"}.`,
		);
	};

	const stopSession = async (sessionId: string) => {
		setStoppingId(sessionId);
		try {
			await stopPodPortForward(client, sessionId);
			await queryClient.invalidateQueries({ queryKey: queryKeys.portForwards() });
		} finally {
			setStoppingId(null);
		}
	};

	const copySessionUrl = async (session: PortForwardSessionSummary) => {
		setCopyingId(session.id);
		try {
			await copyText(portForwardLocalUrl(session));
		} finally {
			setCopyingId(null);
		}
	};

	return (
		<div className="mx-auto flex w-full max-w-6xl flex-col gap-4">
			<header className="flex flex-wrap items-center justify-between gap-3 border-b pb-3">
				<div className="min-w-0">
					<h1 className="truncate text-lg font-semibold">Port Forwards</h1>
					<p className="mt-1 text-xs text-muted-foreground">
						Active local tunnels and saved Service launch presets for{" "}
						{workspace.name}.
					</p>
				</div>
				<div className="flex flex-wrap items-center gap-2">
					<span className="inline-flex items-center gap-2 rounded-md border bg-background/50 px-2 py-1 text-xs text-muted-foreground">
						<ToggleButton
							checked={autoStartSavedPortForwards}
							onCheckedChange={setAutoStartSavedPortForwards}
							ariaLabel="Auto-start saved port forwards"
						/>
						Auto-start saved
					</span>
					<Button
						type="button"
						variant="outline"
						onClick={() => void handleStartAll()}
						disabled={startingAll || savedPortForwards.length === 0}
					>
						{startingAll ? (
							<Spinner data-icon="inline-start" />
						) : (
							<Play data-icon="inline-start" />
						)}
						Start saved
					</Button>
					<Button type="button" onClick={beginAdd}>
						<Plus data-icon="inline-start" />
						Save forward
					</Button>
				</div>
			</header>

			{bulkMessage && (
				<Alert>
					<Cable className="size-3.5" />
					<AlertTitle>Start saved forwards</AlertTitle>
					<AlertDescription>{bulkMessage}</AlertDescription>
				</Alert>
			)}

			<Card>
				<CardHeader>
					<CardTitle className="inline-flex items-center gap-2">
						Active sessions
						{sessionsQuery.isFetching && (
							<Spinner className="size-3.5 text-muted-foreground" />
						)}
					</CardTitle>
					<CardDescription>
						Sessions are in-memory only and bound to 127.0.0.1.
					</CardDescription>
				</CardHeader>
				<CardContent className="flex flex-col gap-3">
					{sessions.length === 0 ? (
						<Empty className="min-h-36 border-0">
							<EmptyHeader>
								<EmptyTitle>No active port forwards</EmptyTitle>
								<EmptyDescription>
									Start a saved forward or open a Pod or Service detail panel.
								</EmptyDescription>
							</EmptyHeader>
						</Empty>
					) : (
						<div className="grid gap-2">
							{sessions.map((session) => {
								const localUrl = portForwardLocalUrl(session);
								return (
									<div
										key={session.id}
										className="grid gap-3 rounded-md border bg-background p-3 md:grid-cols-[minmax(0,1fr)_auto]"
									>
										<div className="min-w-0">
											<div className="flex flex-wrap items-center gap-2">
												<span className="truncate text-sm font-medium">
													{activeSessionTitle(session)}
												</span>
												<Badge
													variant={
														session.status === "error"
															? "destructive"
															: "outline"
													}
												>
													{session.status}
												</Badge>
											</div>
											<div className="mt-1 truncate font-mono text-xs text-muted-foreground">
												{localUrl}
											</div>
											<div className="mt-1 truncate text-xs text-muted-foreground">
												Resolved Pod: {session.resolvedPodName}:
												{session.resolvedPodPort}
											</div>
											{showKubeconfigSourceLabels &&
												session.kubeconfigSourceLabel && (
													<div className="mt-1 truncate text-xs text-muted-foreground">
														{session.kubeconfigSourceLabel}
													</div>
												)}
											{session.lastError && (
												<div className="mt-2 text-xs text-destructive">
													{session.lastError}
												</div>
											)}
										</div>
										<div className="flex flex-wrap items-center justify-end gap-2">
											<Button
												type="button"
												variant="outline"
												size="sm"
												onClick={() => void reconnectSession(session)}
												disabled={reconnectingId === session.id}
											>
												{reconnectingId === session.id ? (
													<Spinner data-icon="inline-start" />
												) : (
													<RotateCcw data-icon="inline-start" />
												)}
												Reconnect
											</Button>
											<Button
												type="button"
												variant="outline"
												size="sm"
												onClick={() => void copySessionUrl(session)}
												disabled={copyingId === session.id}
											>
												<Copy data-icon="inline-start" />
												Copy
											</Button>
											<Button
												type="button"
												variant="outline"
												size="sm"
												onClick={() => void stopSession(session.id)}
												disabled={stoppingId === session.id}
											>
												{stoppingId === session.id ? (
													<Spinner data-icon="inline-start" />
												) : (
													<Square data-icon="inline-start" />
												)}
												Stop
											</Button>
										</div>
									</div>
								);
							})}
						</div>
					)}
				</CardContent>
			</Card>

			<Card>
				<CardHeader>
					<CardTitle>Saved Service forwards</CardTitle>
					<CardDescription>
						Presets resolve the Service to a ready backing Pod each time they
						start.
					</CardDescription>
				</CardHeader>
				<CardContent className="flex flex-col gap-4">
					{formOpen && (
						<div className="rounded-md border bg-background p-4">
							<FieldGroup className="grid gap-4 md:grid-cols-2">
								<Field>
									<FieldLabel htmlFor="saved-pf-label">Label</FieldLabel>
									<Input
										id="saved-pf-label"
										value={form.label}
										placeholder="Optional"
										onChange={(event) => updateForm("label", event.target.value)}
									/>
								</Field>
								<Field>
									<FieldLabel htmlFor="saved-pf-context">
										Cluster context
									</FieldLabel>
									<Input
										id="saved-pf-context"
										value={form.clusterContext}
										onChange={(event) =>
											updateForm("clusterContext", event.target.value)
										}
									/>
								</Field>
								<Field>
									<FieldLabel htmlFor="saved-pf-namespace">
										Namespace
									</FieldLabel>
									<Input
										id="saved-pf-namespace"
										value={form.namespace}
										onChange={(event) =>
											updateForm("namespace", event.target.value)
										}
									/>
								</Field>
								<Field>
									<FieldLabel htmlFor="saved-pf-service">
										Service name
									</FieldLabel>
									<Input
										id="saved-pf-service"
										value={form.serviceName}
										onChange={(event) =>
											updateForm("serviceName", event.target.value)
										}
									/>
								</Field>
								<Field>
									<FieldLabel htmlFor="saved-pf-service-port">
										Service port
									</FieldLabel>
									<Input
										id="saved-pf-service-port"
										inputMode="numeric"
										value={form.servicePort}
										placeholder="Required"
										onChange={(event) =>
											updateForm("servicePort", event.target.value)
										}
									/>
									<FieldDescription>
										Kubernetes resolves this Service port to a Pod target port.
									</FieldDescription>
								</Field>
								<Field>
									<FieldLabel htmlFor="saved-pf-local-port">
										Local port
									</FieldLabel>
									<Input
										id="saved-pf-local-port"
										inputMode="numeric"
										value={form.localPort}
										placeholder="Auto"
										onChange={(event) =>
											updateForm("localPort", event.target.value)
										}
									/>
									<FieldDescription>
										Blank picks an open localhost port. Fixed ports must be
										available.
									</FieldDescription>
								</Field>
							</FieldGroup>
							{formError && (
								<Alert variant="destructive" className="mt-4">
									<AlertTitle>Check saved forward</AlertTitle>
									<AlertDescription>{formError}</AlertDescription>
								</Alert>
							)}
							<div className="mt-4 flex justify-end gap-2">
								<Button type="button" variant="outline" onClick={resetForm}>
									<X data-icon="inline-start" />
									Cancel
								</Button>
								<Button type="button" onClick={submitForm}>
									<Save data-icon="inline-start" />
									{editingId ? "Save changes" : "Save forward"}
								</Button>
							</div>
						</div>
					)}

					{savedPortForwards.length === 0 ? (
						<Empty className="min-h-40 border-0">
							<EmptyHeader>
								<EmptyTitle>No saved forwards</EmptyTitle>
								<EmptyDescription>
									Save selector-backed Service forwards to launch them again from
									this workspace.
								</EmptyDescription>
							</EmptyHeader>
						</Empty>
					) : (
						<div className="grid gap-2">
							{savedPortForwards.map((portForward) => {
								const activeSession = sessions.find((session) =>
									savedPortForwardMatchesSession(
										portForward,
										session,
										kubeconfigEnvVar,
									),
								);
								const starting = startingIds.has(portForward.id);
								return (
									<div
										key={portForward.id}
										className="grid gap-3 rounded-md border bg-background p-3 md:grid-cols-[minmax(0,1fr)_auto]"
									>
										<div className="min-w-0">
											<div className="flex flex-wrap items-center gap-2">
												<span className="truncate text-sm font-medium">
													{savedPortForwardLabel(portForward)}
												</span>
												{activeSession && (
													<Badge variant="secondary">active</Badge>
												)}
												{portForward.lastStatus && (
													<Badge
														variant={
															portForward.lastStatus === "error"
																? "destructive"
																: "outline"
														}
													>
														{portForward.lastStatus}
													</Badge>
												)}
											</div>
											<div className="mt-1 truncate text-xs text-muted-foreground">
												{portForward.clusterContext} / {portForward.namespace}{" "}
												/ Service/{portForward.serviceName}:
												{portForward.servicePort}
											</div>
											<div className="mt-1 text-xs text-muted-foreground">
												Local port: {portForward.localPort ?? "Auto"}
											</div>
											{activeSession && (
												<div className="mt-1 truncate font-mono text-xs text-muted-foreground">
													{portForwardLocalUrl(activeSession)}
												</div>
											)}
											{portForward.lastError && (
												<div className="mt-2 text-xs text-destructive">
													{portForward.lastError}
												</div>
											)}
										</div>
										<div className="flex flex-wrap items-center justify-end gap-2">
											<Button
												type="button"
												variant="outline"
												size="sm"
												disabled={starting}
												onClick={() => void startOne(portForward)}
											>
												{starting ? (
													<Spinner data-icon="inline-start" />
												) : (
													<Play data-icon="inline-start" />
												)}
												Start
											</Button>
											<Button
												type="button"
												variant="outline"
												size="sm"
												onClick={() => beginEdit(portForward)}
											>
												<Pencil data-icon="inline-start" />
												Edit
											</Button>
											<Button
												type="button"
												variant="outline"
												size="sm"
												onClick={() =>
													deleteSavedPortForward(workspace.id, portForward.id)
												}
											>
												<Trash2 data-icon="inline-start" />
												Delete
											</Button>
										</div>
									</div>
								);
							})}
						</div>
					)}
				</CardContent>
			</Card>

			<Separator />
		</div>
	);
}
