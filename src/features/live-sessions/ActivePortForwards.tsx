import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Cable, Copy, RotateCcw, Square, Terminal } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Popover,
	PopoverContent,
	PopoverTrigger,
} from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import {
	createTauriClient,
	listPodExecSessions,
	listPortForwards,
	stopPodExecSession,
	stopPodPortForward,
} from "@/lib/tauri";
import { useSettingsState } from "@/lib/settings";
import type {
	PodExecSessionSummary,
	PortForwardSessionSummary,
} from "@/lib/types";
import { queryKeys } from "@/lib/queryKeys";
import {
	portForwardErrorMessage,
	portForwardLocalUrl,
	sortPortForwardSessions,
} from "./helpers";
import { useReconnectPortForwardSession } from "./useReconnectPortForwardSession";

function sessionLabel(session: PortForwardSessionSummary): string {
	return `${session.namespace}/${session.targetKind}/${session.targetName}:${session.remotePort}`;
}

function execSessionLabel(session: PodExecSessionSummary): string {
	return `${session.namespace}/Pod/${session.podName}`;
}

async function copyText(text: string): Promise<void> {
	await navigator.clipboard?.writeText(text);
}

interface ActivePortForwardsProps {
	onOpenManager?: () => void;
}

export function ActivePortForwards({ onOpenManager }: ActivePortForwardsProps) {
	const client = useMemo(() => createTauriClient(), []);
	const queryClient = useQueryClient();
	const [popoverOpen, setPopoverOpen] = useState(false);
	const [stoppingId, setStoppingId] = useState<string | null>(null);
	const [reconnectError, setReconnectError] = useState<string | null>(null);
	const [copyingId, setCopyingId] = useState<string | null>(null);
	const showKubeconfigSourceLabels = useSettingsState(
		(state) => state.showKubeconfigSourceLabels,
	);
	const { reconnectingId, reconnectSession } =
		useReconnectPortForwardSession({
			client,
			onSuccess: () => setReconnectError(null),
			onError: (error) => setReconnectError(portForwardErrorMessage(error)),
		});
	const sessionsQuery = useQuery({
		queryKey: queryKeys.portForwards(),
		queryFn: () => listPortForwards(client),
		placeholderData: (previousData) => previousData,
		refetchInterval: 3_000,
	});
	const execSessionsQuery = useQuery({
		queryKey: queryKeys.podExecSessions(),
		queryFn: () => listPodExecSessions(client),
		placeholderData: (previousData) => previousData,
		refetchInterval: 3_000,
	});
	const sessions = useMemo(
		() => sortPortForwardSessions(sessionsQuery.data ?? []),
		[sessionsQuery.data],
	);
	const execSessions = useMemo(
		() =>
			(execSessionsQuery.data ?? []).toSorted((a, b) =>
				`${a.clusterContext}:${a.namespace}:${a.podName}:${a.startedAt}`.localeCompare(
					`${b.clusterContext}:${b.namespace}:${b.podName}:${b.startedAt}`,
				),
			),
		[execSessionsQuery.data],
	);
	const sessionCount = sessions.length + execSessions.length;

	if (
		sessionCount === 0 &&
		!sessionsQuery.isLoading &&
		!execSessionsQuery.isLoading
	) {
		return null;
	}

	const stopSession = async (sessionId: string) => {
		setStoppingId(sessionId);
		try {
			await stopPodPortForward(client, sessionId);
			await queryClient.invalidateQueries({ queryKey: queryKeys.portForwards() });
		} finally {
			setStoppingId(null);
		}
	};

	const stopExecSession = async (sessionId: string) => {
		setStoppingId(sessionId);
		try {
			await stopPodExecSession(client, sessionId);
			await queryClient.invalidateQueries({
				queryKey: queryKeys.podExecSessions(),
			});
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
		<Popover open={popoverOpen} onOpenChange={setPopoverOpen}>
			<PopoverTrigger asChild>
				<Button
					type="button"
					variant="ghost"
					size="sm"
					className="mr-1 h-8 text-muted-foreground [-webkit-app-region:no-drag]"
				>
					<Cable data-icon="inline-start" />
					{sessionCount}
				</Button>
			</PopoverTrigger>
			<PopoverContent align="end" className="w-96">
				<div className="flex flex-col gap-3">
					<div className="flex items-center justify-between gap-3">
						<div className="min-w-0">
							<div className="truncate text-sm font-semibold">
								Live sessions
							</div>
							<div className="text-xs text-muted-foreground">
								Active port-forwards and Pod exec sessions
							</div>
						</div>
						{(sessionsQuery.isFetching || execSessionsQuery.isFetching) && (
							<Spinner className="size-3.5" />
						)}
					</div>
					{onOpenManager && (
						<Button
							type="button"
							variant="outline"
							size="sm"
							onClick={() => {
								setPopoverOpen(false);
								onOpenManager();
							}}
						>
							<Cable data-icon="inline-start" />
							Manage
						</Button>
					)}
					{reconnectError && (
						<div className="text-xs text-destructive">{reconnectError}</div>
					)}
					<Separator />
					<div className="flex max-h-80 flex-col gap-2 overflow-y-auto">
						{execSessions.map((session) => (
							<div
								key={session.id}
								className="flex min-w-0 flex-col gap-2 rounded-md border bg-background p-2"
							>
								<div className="flex min-w-0 items-center justify-between gap-2">
									<div className="min-w-0">
										<div className="inline-flex max-w-full items-center gap-1 truncate text-xs font-medium">
											<Terminal className="size-3 shrink-0" />
											<span className="truncate">{execSessionLabel(session)}</span>
										</div>
										<div className="truncate font-mono text-[11px] text-muted-foreground">
											{session.command.join(" ")}
										</div>
										{showKubeconfigSourceLabels &&
											session.kubeconfigSourceLabel && (
												<div className="truncate text-[11px] text-muted-foreground">
													{session.kubeconfigSourceLabel}
												</div>
											)}
									</div>
									<Badge
										variant={
											session.status === "error" ? "destructive" : "outline"
										}
									>
										{session.status}
									</Badge>
								</div>
								{session.lastError && (
									<div className="text-xs text-destructive">
										{session.lastError}
									</div>
								)}
								<div className="flex flex-wrap justify-end gap-2">
									<Button
										type="button"
										variant="outline"
										size="sm"
										onClick={() => void stopExecSession(session.id)}
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
						{sessions.map((session) => {
							const localUrl = portForwardLocalUrl(session);
							return (
								<div
									key={session.id}
									className="flex min-w-0 flex-col gap-2 rounded-md border bg-background p-2"
								>
									<div className="flex min-w-0 items-center justify-between gap-2">
										<div className="min-w-0">
											<div className="truncate text-xs font-medium">
												{sessionLabel(session)}
											</div>
											<div className="truncate font-mono text-[11px] text-muted-foreground">
												{localUrl}
											</div>
											{showKubeconfigSourceLabels &&
												session.kubeconfigSourceLabel && (
													<div className="truncate text-[11px] text-muted-foreground">
														{session.kubeconfigSourceLabel}
													</div>
												)}
										</div>
										<Badge
											variant={
												session.status === "error" ? "destructive" : "outline"
											}
										>
											{session.status}
										</Badge>
									</div>
									{session.lastError && (
										<div className="text-xs text-destructive">
											{session.lastError}
										</div>
									)}
									<div className="flex flex-wrap justify-end gap-2">
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
						{sessionCount === 0 && (
							<div className="text-sm text-muted-foreground">
								Loading active sessions...
							</div>
						)}
					</div>
				</div>
			</PopoverContent>
		</Popover>
	);
}
