import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowDownUp, RotateCcw, Search } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { TimestampText } from "@/components/TimestampText";
import { formatExactTimeOnly } from "@/components/timestamp-format";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
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
import { useSettingsState } from "@/lib/settings";
import type { TauriClient } from "@/lib/tauri";
import type { ResourceSummary } from "@/lib/types";
import { cn } from "@/lib/utils";
import type { ContainerStatusRow } from "./helpers";
import {
	latestTimestampedLogLine,
	orderedLogLines,
	type ParsedLogLine,
} from "./log-helpers";
import { usePodLogStream } from "./usePodLogStream";

function LogLineTime({ value }: { value: string }) {
	const timestampTimezone = useSettingsState(
		(state) => state.timestampTimezone,
	);
	const timeOnly = formatExactTimeOnly(value, timestampTimezone);
	// Time-of-day inline; the full date stays one hover away in the tooltip.
	return (
		<TimestampText
			relative={timeOnly ?? value}
			exact={value}
			precision="millisecond"
		/>
	);
}

interface LogsTabProps {
	client: TauriClient;
	resource: ResourceSummary;
	containers: ContainerStatusRow[];
	selectedContainer: string;
	onSelectedContainerChange: (container: string) => void;
	onLatestLogLineChange?: (line: ParsedLogLine | undefined) => void;
	active: boolean;
}

export function LogsTab({
	client,
	resource,
	containers,
	selectedContainer,
	onSelectedContainerChange,
	onLatestLogLineChange,
	active,
}: LogsTabProps) {
	const [wrapLines, setWrapLines] = useState(true);
	const [latestFirst, setLatestFirst] = useState(false);
	const [autoFollow, setAutoFollow] = useState(true);
	const [filter, setFilter] = useState("");
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	const logViewportRef = useRef<HTMLDivElement>(null);
	const regularContainers = containers.filter(
		(container) => container.type !== "init",
	);
	const options =
		regularContainers.length > 0 ? regularContainers : containers;
	const request = useMemo(() => {
		if (resource.kind !== "Pod" || !resource.namespace || !selectedContainer) {
			return null;
		}
		return {
			clusterContext: resource.cluster,
			kubeconfigEnvVar,
			namespace: resource.namespace,
			podName: resource.name,
			container: selectedContainer,
			tailLines: 200,
		};
	}, [
		resource.cluster,
		resource.kind,
		resource.name,
		resource.namespace,
		kubeconfigEnvVar,
		selectedContainer,
	]);
	const logStream = usePodLogStream({
		client,
		request,
		enabled: active && request !== null,
	});
	const parsedLogLines = useMemo(
		() => orderedLogLines(logStream.lines, false),
		[logStream.lines],
	);
	const orderedLines = useMemo(
		() => (latestFirst ? [...parsedLogLines].reverse() : parsedLogLines),
		[latestFirst, parsedLogLines],
	);
	const filterTerm = filter.trim().toLowerCase();
	const visibleLines = useMemo(() => {
		if (!filterTerm) return orderedLines;
		return orderedLines.filter((line) =>
			line.raw.toLowerCase().includes(filterTerm),
		);
	}, [filterTerm, orderedLines]);
	const latestLogLine = useMemo(
		() => latestTimestampedLogLine(parsedLogLines),
		[parsedLogLines],
	);
	useEffect(() => {
		onLatestLogLineChange?.(latestLogLine);
	}, [latestLogLine, onLatestLogLineChange]);
	useEffect(() => {
		if (!autoFollow || logStream.lines.length === 0) return;
		const viewport = logViewportRef.current;
		if (!viewport) return;

		const frame = window.requestAnimationFrame(() => {
			viewport.scrollTop = latestFirst ? 0 : viewport.scrollHeight;
		});

		return () => window.cancelAnimationFrame(frame);
	}, [autoFollow, latestFirst, logStream.lines.length, logStream.version, wrapLines]);

	if (resource.kind !== "Pod") {
		return (
			<Empty className="min-h-64 border-0">
				<EmptyHeader>
					<EmptyTitle>Logs available for Pods</EmptyTitle>
					<EmptyDescription>
						Select a Pod to tail read-only container logs.
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
					Pod logs require a namespaced Pod target.
				</AlertDescription>
			</Alert>
		);
	}

	if (options.length === 0) {
		return (
			<Empty className="min-h-64 border-0">
				<EmptyHeader>
					<EmptyTitle>No containers found</EmptyTitle>
					<EmptyDescription>
						Container status has not loaded for this Pod yet.
					</EmptyDescription>
				</EmptyHeader>
			</Empty>
		);
	}

	return (
		<div className="flex h-full min-h-0 flex-col gap-3">
			{logStream.error && (
				<Alert variant="destructive">
					<AlertTitle>Failed to stream logs</AlertTitle>
					<AlertDescription>{logStream.error}</AlertDescription>
				</Alert>
			)}
			<div
				ref={logViewportRef}
				className="min-h-0 flex-1 overflow-auto rounded-md border bg-muted/20"
			>
				<div className="sticky top-0 left-0 z-20 flex min-w-full flex-wrap items-center justify-between gap-2 border-b bg-card/95 px-3 py-2 backdrop-blur supports-[backdrop-filter]:bg-card/85">
					<div className="flex min-w-0 items-center gap-2">
						<Select
							value={selectedContainer}
							onValueChange={onSelectedContainerChange}
						>
							<SelectTrigger size="sm" aria-label="Container">
								<SelectValue placeholder="Container" />
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
						<Badge
							variant={logStream.status === "error" ? "destructive" : "outline"}
						>
							Logs: {logStream.status}
						</Badge>
						<div className="relative min-w-0">
							<Search className="pointer-events-none absolute left-2 top-1/2 size-3 -translate-y-1/2 text-muted-foreground" />
							<Input
								value={filter}
								onChange={(event) => setFilter(event.target.value)}
								placeholder="Filter logs..."
								aria-label="Filter log lines"
								className="h-6 w-40 pl-6 text-xs"
							/>
						</div>
						{filterTerm && (
							<Badge variant="outline">
								{visibleLines.length}/{parsedLogLines.length}
							</Badge>
						)}
					</div>
					<div className="flex flex-wrap items-center gap-2">
						<Label
							htmlFor="wrap-log-lines"
							className="h-6 gap-1.5 rounded-md border bg-background px-2 text-xs text-muted-foreground"
						>
							<Checkbox
								id="wrap-log-lines"
								className="size-3.5"
								checked={wrapLines}
								onCheckedChange={(checked) => setWrapLines(checked === true)}
							/>
							Wrap lines
						</Label>
						<Label
							htmlFor="auto-follow-logs"
							className="h-6 gap-1.5 rounded-md border bg-background px-2 text-xs text-muted-foreground"
						>
							<Checkbox
								id="auto-follow-logs"
								className="size-3.5"
								checked={autoFollow}
								onCheckedChange={(checked) => setAutoFollow(checked === true)}
							/>
							Follow
						</Label>
						<Button
							type="button"
							variant="outline"
							size="sm"
							aria-pressed={latestFirst}
							onClick={() => setLatestFirst((value) => !value)}
						>
							<ArrowDownUp data-icon="inline-start" />
							{latestFirst ? "Latest top" : "Oldest top"}
						</Button>
						<Button
							type="button"
							variant="outline"
							size="sm"
							onClick={logStream.clear}
						>
							<RotateCcw data-icon="inline-start" />
							Clear
						</Button>
					</div>
				</div>
				{logStream.lines.length === 0 ? (
					<div className="flex items-center gap-2 p-3 text-muted-foreground">
						{logStream.status === "connecting" && <Spinner className="size-3.5" />}
						<span>{logStream.message}</span>
					</div>
				) : visibleLines.length === 0 ? (
					<div className="p-3 text-muted-foreground">
						No log lines match “{filter.trim()}”.
					</div>
				) : (
					<div
						className={cnfast(
							"min-w-full font-mono text-xs leading-5",
							!wrapLines && "w-max",
						)}
					>
						{visibleLines.map((line) => (
							<div
								key={`${line.index}:${line.raw}`}
								className={cnfast(
									"flex min-w-full border-b border-border/50 last:border-b-0",
									!wrapLines && "w-max",
								)}
							>
								<time
									className="w-32 shrink-0 whitespace-nowrap border-r border-border/50 px-3 py-1 text-muted-foreground tabular-nums"
									dateTime={line.timestamp}
								>
									{line.timestamp ? (
										<LogLineTime value={line.timestamp} />
									) : (
										"—"
									)}
								</time>
								<code
									className={cn(
										"block px-3 py-1 text-foreground",
										wrapLines
											? "min-w-0 flex-1 whitespace-pre-wrap break-words"
											: "whitespace-pre",
									)}
								>
									{line.message}
								</code>
							</div>
						))}
					</div>
				)}
			</div>
		</div>
	);
}
