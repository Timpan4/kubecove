import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Cable, Copy, Play, Square } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import {
	listPortForwards,
	startPodPortForward,
	stopPodPortForward,
	type TauriClient,
} from "@/lib/tauri";
import type { PortForwardSessionSummary, ResourceSummary } from "@/lib/types";
import { queryKeys } from "@/lib/queryKeys";
import { getErrorMessage } from "./helpers";
import {
	isPortForwardForResource,
	parsePortForwardForm,
	portForwardLocalUrl,
	sortPortForwardSessions,
} from "@/features/live-sessions/helpers";

interface PortForwardTabProps {
	client: TauriClient;
	resource: ResourceSummary;
	active: boolean;
}

async function copyText(text: string): Promise<void> {
	await navigator.clipboard?.writeText(text);
}

function sessionTitle(session: PortForwardSessionSummary): string {
	if (session.targetKind === "Service") {
		return `${session.localAddress}:${session.localPort} -> Service/${session.targetName}:${session.remotePort}`;
	}
	return `${session.localAddress}:${session.localPort} -> Pod/${session.targetName}:${session.remotePort}`;
}

function sessionResolution(session: PortForwardSessionSummary): string {
	if (session.targetKind === "Service") {
		return `Resolved to Pod/${session.resolvedPodName}:${session.resolvedPodPort}`;
	}
	return `Pod/${session.resolvedPodName}:${session.resolvedPodPort}`;
}

function isForwardableResource(resource: ResourceSummary): boolean {
	return resource.kind === "Pod" || resource.kind === "Service";
}

export function PortForwardTab({
	client,
	resource,
	active,
}: PortForwardTabProps) {
	const queryClient = useQueryClient();
	const [remotePort, setRemotePort] = useState("");
	const [localPort, setLocalPort] = useState("");
	const [formError, setFormError] = useState<string | null>(null);
	const [startError, setStartError] = useState<string | null>(null);
	const [starting, setStarting] = useState(false);
	const [stoppingId, setStoppingId] = useState<string | null>(null);
	const [copyingId, setCopyingId] = useState<string | null>(null);
	const sessionsQuery = useQuery({
		queryKey: queryKeys.portForwards(),
		queryFn: () => listPortForwards(client),
		enabled: active,
		refetchInterval: active ? 3_000 : false,
	});
	const sessions = useMemo(
		() =>
			sortPortForwardSessions(
				(sessionsQuery.data ?? []).filter((session) =>
					isPortForwardForResource(session, resource),
				),
			),
		[resource, sessionsQuery.data],
	);

	if (!isForwardableResource(resource)) {
		return (
			<Empty className="min-h-64 border-0">
				<EmptyHeader>
					<EmptyTitle>Port forwarding starts from Pods or Services</EmptyTitle>
					<EmptyDescription>
						Select a namespaced Pod or selector-backed Service.
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
					Port-forwarding requires a namespaced target.
				</AlertDescription>
			</Alert>
		);
	}
	const targetKind = resource.kind === "Service" ? "Service" : "Pod";

	const invalidateSessions = () =>
		queryClient.invalidateQueries({ queryKey: queryKeys.portForwards() });

	const startSession = async () => {
		setFormError(null);
		setStartError(null);
		const parsed = parsePortForwardForm(
			{ remotePort, localPort },
			{ remotePortLabel: targetKind === "Service" ? "Service port" : "Pod port" },
		);
		if (typeof parsed === "string") {
			setFormError(parsed);
			return;
		}
		setStarting(true);
		try {
			await startPodPortForward(client, {
				clusterContext: resource.cluster,
				namespace: resource.namespace ?? "",
				targetKind,
				targetName: resource.name,
				podName: targetKind === "Pod" ? resource.name : undefined,
				remotePort: parsed.remotePort,
				localPort: parsed.localPort,
			});
			setLocalPort("");
			await invalidateSessions();
		} catch (err) {
			setStartError(getErrorMessage(err));
		} finally {
			setStarting(false);
		}
	};

	const stopSession = async (sessionId: string) => {
		setStoppingId(sessionId);
		try {
			await stopPodPortForward(client, sessionId);
			await invalidateSessions();
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
		<div className="flex flex-col gap-4">
			<Alert>
				<Cable className="size-3.5" />
				<AlertTitle>Local {targetKind} tunnel</AlertTitle>
				<AlertDescription>
					KubeCove binds only to 127.0.0.1 and keeps sessions in memory.
					{targetKind === "Service"
						? " Services resolve once to a ready backing Pod."
						: ""}
				</AlertDescription>
			</Alert>
			<FieldGroup>
				<Field>
					<FieldLabel htmlFor="pod-port-forward-remote-port">
						{targetKind === "Service" ? "Service port" : "Pod port"}
					</FieldLabel>
					<Input
						id="pod-port-forward-remote-port"
						inputMode="numeric"
						value={remotePort}
						placeholder="Required"
						onChange={(event) => setRemotePort(event.target.value)}
					/>
					<FieldDescription>
						{targetKind === "Service"
							? "The Service port Kubernetes should resolve to a backing Pod."
							: "The Pod port Kubernetes should forward to."}
					</FieldDescription>
				</Field>
				<Field>
					<FieldLabel htmlFor="pod-port-forward-local-port">
						Local port
					</FieldLabel>
					<Input
						id="pod-port-forward-local-port"
						inputMode="numeric"
						value={localPort}
						placeholder="Auto"
						onChange={(event) => setLocalPort(event.target.value)}
					/>
					<FieldDescription>
						Optional. Leave empty to choose an open localhost port.
					</FieldDescription>
				</Field>
			</FieldGroup>
			{formError && (
				<Alert variant="destructive">
					<AlertTitle>Check port values</AlertTitle>
					<AlertDescription>{formError}</AlertDescription>
				</Alert>
			)}
			{startError && (
				<Alert variant="destructive">
					<AlertTitle>Failed to start port forward</AlertTitle>
					<AlertDescription>{startError}</AlertDescription>
				</Alert>
			)}
			<div className="flex justify-end">
				<Button type="button" onClick={() => void startSession()} disabled={starting}>
					{starting ? (
						<Spinner data-icon="inline-start" />
					) : (
						<Play data-icon="inline-start" />
					)}
					Start
				</Button>
			</div>
			<Separator />
			<div className="flex flex-col gap-3">
				<div className="flex items-center justify-between gap-3">
					<div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
						Active sessions
					</div>
					{sessionsQuery.isFetching && <Spinner className="size-3.5" />}
				</div>
				{sessions.length === 0 ? (
					<Empty className="min-h-40 border-0">
						<EmptyHeader>
							<EmptyTitle>No port forwards</EmptyTitle>
							<EmptyDescription>
								Start a tunnel to make this {targetKind} reachable on localhost.
							</EmptyDescription>
						</EmptyHeader>
					</Empty>
				) : (
					<div className="flex flex-col gap-2">
						{sessions.map((session) => {
							const localUrl = portForwardLocalUrl(session);
							return (
								<div
									key={session.id}
									className="flex flex-col gap-2 rounded-md border bg-background p-3"
								>
									<div className="flex min-w-0 items-center justify-between gap-2">
										<div className="min-w-0">
											<div className="truncate text-xs font-medium">
												{sessionTitle(session)}
											</div>
											<div className="truncate font-mono text-[11px] text-muted-foreground">
												{localUrl}
											</div>
											<div className="truncate text-[11px] text-muted-foreground">
												{sessionResolution(session)}
											</div>
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
			</div>
		</div>
	);
}
