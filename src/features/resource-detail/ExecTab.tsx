import "@xterm/xterm/css/xterm.css";

import { useEffect, useMemo, useRef, useState } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { Play, RotateCcw, Square, Terminal as TerminalIcon } from "lucide-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
import { Label } from "@/components/ui/label";
import {
	Select,
	SelectContent,
	SelectGroup,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import {
	closePodExecChannel,
	createPodExecChannel,
	listPodExecSessions,
	resizePodExecTerminal,
	startPodExecSession,
	stopPodExecSession,
	writePodExecStdin,
	type TauriClient,
} from "@/lib/tauri";
import { queryKeys } from "@/lib/queryKeys";
import { useSettingsState } from "@/lib/settings";
import type { PodExecSessionMessage, ResourceSummary } from "@/lib/types";
import { cnfast } from "@/lib/utils";
import type { ContainerStatusRow } from "./helpers";
import { getErrorMessage } from "./helpers";
import {
	buildPodExecRequest,
	commandForPreset,
	isPodExecForResource,
	podExecCommandText,
	podExecTarget,
	sortPodExecSessions,
	type PodExecPreset,
} from "./pod-exec-helpers";

interface ExecTabProps {
	client: TauriClient;
	resource: ResourceSummary;
	containers: ContainerStatusRow[];
	selectedContainer: string;
	onSelectedContainerChange: (container: string) => void;
	active: boolean;
}

const DEFAULT_COLS = 100;
const DEFAULT_ROWS = 32;

export function ExecTab({
	client,
	resource,
	containers,
	selectedContainer,
	onSelectedContainerChange,
	active,
}: ExecTabProps) {
	const queryClient = useQueryClient();
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	const showKubeconfigSourceLabels = useSettingsState(
		(state) => state.showKubeconfigSourceLabels,
	);
	const terminalHostRef = useRef<HTMLDivElement>(null);
	const terminalRef = useRef<Terminal | null>(null);
	const fitAddonRef = useRef<FitAddon | null>(null);
	const channelRef = useRef<ReturnType<typeof createPodExecChannel> | null>(null);
	const sessionIdRef = useRef<string | null>(null);
	const channelTokenRef = useRef(0);
	const [preset, setPreset] = useState<PodExecPreset>("sh");
	const [customArgv, setCustomArgv] = useState("");
	const [confirmed, setConfirmed] = useState(false);
	const [starting, setStarting] = useState(false);
	const [stoppingId, setStoppingId] = useState<string | null>(null);
	const [status, setStatus] = useState("idle");
	const [message, setMessage] = useState("Start an exec session to open the terminal.");
	const [error, setError] = useState<string | null>(null);
	const options = useMemo(() => {
		const regularContainers = containers.filter(
			(container) => container.type !== "init",
		);
		return regularContainers.length > 0 ? regularContainers : containers;
	}, [containers]);
	const { data: sessionsData, isFetching: sessionsFetching } = useQuery({
		queryKey: queryKeys.podExecSessions(),
		queryFn: () => listPodExecSessions(client),
		enabled: active,
		refetchInterval: active ? 3_000 : false,
	});
	const sessions = useMemo(
		() =>
			sortPodExecSessions(
				(sessionsData ?? []).filter((session) =>
					isPodExecForResource(session, resource, kubeconfigEnvVar),
				),
			),
		[resource, sessionsData, kubeconfigEnvVar],
	);
	const command = commandForPreset(preset, customArgv);
	const commandText = typeof command === "string" ? "" : podExecCommandText(command);

	const closeCurrentSession = () => {
		channelTokenRef.current += 1;
		sessionIdRef.current = null;
		if (channelRef.current) {
			closePodExecChannel(channelRef.current);
			channelRef.current = null;
		}
	};

	useEffect(() => {
		if (!active || !terminalHostRef.current || terminalRef.current) return;
		const terminal = new Terminal({
			cursorBlink: true,
			convertEol: true,
			fontFamily: "var(--font-mono, ui-monospace, SFMono-Regular, Consolas, monospace)",
			fontSize: 12,
			theme: {
				background: "#101113",
				foreground: "#f4f4f5",
			},
		});
		const fitAddon = new FitAddon();
		terminal.loadAddon(fitAddon);
		terminal.open(terminalHostRef.current);
		terminal.onData((data) => {
			const sessionId = sessionIdRef.current;
			if (sessionId) void writePodExecStdin(client, sessionId, data);
		});
		terminal.onResize(({ cols, rows }) => {
			const sessionId = sessionIdRef.current;
			if (sessionId) {
				void resizePodExecTerminal(client, sessionId, { cols, rows });
			}
		});
		terminalRef.current = terminal;
		fitAddonRef.current = fitAddon;
		const fit = () => {
			try {
				fitAddon.fit();
			} catch {
				// The terminal host can be briefly detached while the detail panel resizes.
			}
		};
		const frame = window.requestAnimationFrame(fit);
		window.addEventListener("resize", fit);
		return () => {
			window.cancelAnimationFrame(frame);
			window.removeEventListener("resize", fit);
			terminal.dispose();
			terminalRef.current = null;
			fitAddonRef.current = null;
		};
	}, [active, client]);

	useEffect(() => {
		if (!active) return;
		const frame = window.requestAnimationFrame(() => {
			try {
				fitAddonRef.current?.fit();
			} catch {
				// Ignore transient layout states while the tab becomes visible.
			}
		});
		return () => window.cancelAnimationFrame(frame);
	}, [active]);

	useEffect(() => {
		const sessionId = sessionIdRef.current;
		return () => {
			closeCurrentSession();
			if (sessionId) void stopPodExecSession(client, sessionId);
		};
	}, [client]);

	if (resource.kind !== "Pod") {
		return (
			<Empty className="min-h-64 border-0">
				<EmptyHeader>
					<EmptyTitle>Exec starts from Pods</EmptyTitle>
					<EmptyDescription>
						Select an exact namespaced Pod before opening an exec session.
					</EmptyDescription>
				</EmptyHeader>
			</Empty>
		);
	}

	if (!resource.namespace) {
		return (
			<Alert variant="destructive">
				<AlertTitle>Namespace required</AlertTitle>
				<AlertDescription>
					Pod exec requires a namespaced Pod target.
				</AlertDescription>
			</Alert>
		);
	}

	const invalidateSessions = () =>
		queryClient.invalidateQueries({ queryKey: queryKeys.podExecSessions() });

	const handleMessage = (event: PodExecSessionMessage, token: number) => {
		if (token !== channelTokenRef.current) return;
		if (
			event.type !== "started" &&
			sessionIdRef.current &&
			event.sessionId !== sessionIdRef.current
		) {
			return;
		}
		if (event.type === "started") {
			if (
				sessionIdRef.current &&
				sessionIdRef.current !== event.sessionId
			) {
				return;
			}
			sessionIdRef.current = event.sessionId;
			setStatus(event.summary.status);
			return;
		}
		if (event.type === "status") {
			setStatus(event.status);
			setMessage(event.message);
			return;
		}
		if (event.type === "output") {
			terminalRef.current?.write(event.data);
			return;
		}
		if (event.type === "error") {
			setStatus("error");
			setError(event.message);
			terminalRef.current?.writeln(`\r\n${event.message}`);
			closeCurrentSession();
			void invalidateSessions();
			return;
		}
		if (event.type === "exited") {
			setStatus("exited");
			setMessage(
				event.exitCode === undefined
					? "Exec session exited"
					: `Exec session exited with code ${event.exitCode}`,
			);
			closeCurrentSession();
			void invalidateSessions();
			return;
		}
		if (event.type === "stopped") {
			setMessage("Exec session stopped");
			closeCurrentSession();
			void invalidateSessions();
		}
	};

	const startSession = async () => {
		setError(null);
		const terminal = terminalRef.current;
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
			kubeconfigEnvVar,
		);
		if (typeof request === "string") {
			setError(request);
			return;
		}
		const previousSessionId = sessionIdRef.current;
		closeCurrentSession();
		if (previousSessionId) {
			await stopPodExecSession(client, previousSessionId);
		}
		setStarting(true);
		setStatus("starting");
		setMessage("Starting exec session");
		terminal?.clear();
		terminal?.writeln(
			`Starting ${commandText} on ${podExecTarget(resource, selectedContainer)}\r\n`,
		);
		const channelToken = channelTokenRef.current + 1;
		channelTokenRef.current = channelToken;
		const channel = createPodExecChannel((event) =>
			handleMessage(event, channelToken),
		);
		channelRef.current = channel;
		try {
			const summary = await startPodExecSession(client, request, channel);
			sessionIdRef.current = summary.id;
			await invalidateSessions();
		} catch (err) {
			closePodExecChannel(channel);
			channelRef.current = null;
			setStatus("error");
			setError(getErrorMessage(err));
		}
		setStarting(false);
	};

	const stopSession = async (sessionId: string) => {
		setStoppingId(sessionId);
		try {
			const isActiveSession = sessionIdRef.current === sessionId;
			if (isActiveSession) closeCurrentSession();
			await stopPodExecSession(client, sessionId);
			if (isActiveSession) {
				setStatus("stopped");
			}
			await invalidateSessions();
		} catch (error) {
			setStoppingId(null);
			throw error;
		}
		setStoppingId(null);
	};

	return (
		<div className="flex h-full min-h-0 flex-col gap-4">
			<Alert>
				<TerminalIcon className="size-3.5" />
				<AlertTitle>Guarded Pod exec</AlertTitle>
				<AlertDescription>
					Exec is limited to this exact Pod and runs through the Kubernetes API.
				</AlertDescription>
			</Alert>
			<FieldGroup>
				<Field>
					<FieldLabel htmlFor="pod-exec-container">Container</FieldLabel>
					<Select
						value={selectedContainer}
						onValueChange={(container) => {
							onSelectedContainerChange(container);
							setConfirmed(false);
						}}
					>
						<SelectTrigger id="pod-exec-container" className="w-full">
							<SelectValue placeholder="Default container" />
						</SelectTrigger>
						<SelectContent>
							<SelectGroup>
								{options.map((container) => (
									<SelectItem key={container.name} value={container.name}>
										{container.name}
									</SelectItem>
								))}
							</SelectGroup>
						</SelectContent>
					</Select>
					<FieldDescription>
						Kubernetes chooses the default only when no container is selected.
					</FieldDescription>
				</Field>
				<Field>
					<FieldLabel htmlFor="pod-exec-command">Command</FieldLabel>
					<Select
						value={preset}
						onValueChange={(value) => {
							setPreset(value as PodExecPreset);
							setConfirmed(false);
						}}
					>
						<SelectTrigger id="pod-exec-command" className="w-full">
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
					<FieldDescription>
						Shell presets are exact commands; KubeCove does not fall back.
					</FieldDescription>
				</Field>
				{preset === "custom" && (
					<Field data-invalid={Boolean(error?.includes("argv"))}>
						<FieldLabel htmlFor="pod-exec-custom-argv">Custom argv</FieldLabel>
						<Textarea
							id="pod-exec-custom-argv"
							value={customArgv}
							placeholder={"/usr/bin/env\nprintenv"}
							aria-invalid={Boolean(error?.includes("argv"))}
							onChange={(event) => {
								setCustomArgv(event.target.value);
								setConfirmed(false);
							}}
						/>
						<FieldDescription>
							Enter one argv item per line. This is not parsed as a local shell.
						</FieldDescription>
					</Field>
				)}
			</FieldGroup>
			<div className="rounded-md border bg-muted/20 p-3 text-xs">
				<div className="font-medium">Target</div>
				<div className="mt-1 font-mono text-muted-foreground">
					{podExecTarget(resource, selectedContainer)}
				</div>
				<div className="mt-3 font-medium">Command</div>
				<div className="mt-1 font-mono text-muted-foreground">
					{commandText || "Custom argv is required"}
				</div>
			</div>
			<Label
				htmlFor="pod-exec-confirm"
				className="gap-2 rounded-md border bg-background p-3 text-xs text-muted-foreground"
			>
				<Checkbox
					id="pod-exec-confirm"
					checked={confirmed}
					onCheckedChange={(checked) => setConfirmed(checked === true)}
				/>
				I understand this opens an interactive session in the selected Pod.
			</Label>
			{error && (
				<Alert variant="destructive">
					<AlertTitle>Exec session failed</AlertTitle>
					<AlertDescription>{error}</AlertDescription>
				</Alert>
			)}
			<div className="flex flex-wrap items-center justify-between gap-2">
				<Badge variant={status === "error" ? "destructive" : "outline"}>
					Exec: {status}
				</Badge>
				<div className="text-xs text-muted-foreground">{message}</div>
				<Button type="button" onClick={() => void startSession()} disabled={starting}>
					{starting ? (
						<Spinner data-icon="inline-start" />
					) : (
						<Play data-icon="inline-start" />
					)}
					Start
				</Button>
			</div>
			<div
				ref={terminalHostRef}
				className={cnfast(
					"min-h-[280px] flex-1 overflow-hidden rounded-md border bg-background p-2",
					!active && "hidden",
				)}
			/>
			<div className="flex flex-col gap-2">
				<div className="flex items-center justify-between gap-3">
					<div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
						Active exec sessions
					</div>
					{sessionsFetching && <Spinner className="size-3.5" />}
				</div>
				{sessions.length === 0 ? (
					<Empty className="min-h-32 border-0">
						<EmptyHeader>
							<EmptyTitle>No exec sessions</EmptyTitle>
							<EmptyDescription>
								Start a guarded exec session for this Pod.
							</EmptyDescription>
						</EmptyHeader>
					</Empty>
				) : (
					<div className="flex flex-col gap-2">
						{sessions.map((session) => (
							<div
								key={session.id}
								className="flex flex-wrap items-center justify-between gap-2 rounded-md border bg-background p-3"
							>
								<div className="min-w-0">
									<div className="truncate font-mono text-xs">
										{podExecCommandText(session.command)}
									</div>
									<div className="truncate text-xs text-muted-foreground">
										{session.container
											? `Container ${session.container}`
											: "Default container"}
									</div>
									{showKubeconfigSourceLabels &&
										session.kubeconfigSourceLabel && (
											<div className="truncate text-xs text-muted-foreground">
												{session.kubeconfigSourceLabel}
											</div>
										)}
								</div>
								<div className="flex items-center gap-2">
									<Badge
										variant={
											session.status === "error" ? "destructive" : "outline"
										}
									>
										{session.status}
									</Badge>
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
						))}
					</div>
				)}
			</div>
			<Button
				type="button"
				variant="outline"
				size="sm"
				onClick={() => terminalRef.current?.clear()}
			>
				<RotateCcw data-icon="inline-start" />
				Clear terminal
			</Button>
		</div>
	);
}
