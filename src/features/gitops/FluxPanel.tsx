import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
import { Spinner } from "@/components/ui/spinner";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { StatusBadge } from "@/components/StatusBadge";
import { TimestampText } from "@/components/TimestampText";
import { queryKeys } from "@/lib/queryKeys";
import { useSettingsState } from "@/lib/settings";
import { createTauriClient, detectFlux, listFluxResources } from "@/lib/tauri";
import type { FluxResourceSummary } from "@/lib/types";
import { cn } from "@/lib/utils";
import { fluxKindFromLabel, fluxStatusTone } from "./flux-kinds";

const STATE_CLASS =
	"flex min-h-64 items-center justify-center p-8 text-center text-sm text-muted-foreground";
const TABLE_CLASS =
	"w-full table-fixed border-collapse text-sm [&_th]:border-b-2 [&_th]:px-3 [&_th]:py-3 [&_th]:text-left [&_th]:text-xs [&_th]:font-semibold [&_th]:uppercase [&_th]:text-muted-foreground [&_td]:whitespace-nowrap [&_td]:border-b [&_td]:px-3 [&_td]:py-3 [&_td]:truncate";
const ROW_CLASS = "cursor-pointer transition-colors hover:bg-accent/60";
const SELECTED_ROW_CLASS = "bg-accent";

export function FluxPanel({
	clusterContext,
	selectedKindLabel,
	selectedFluxResource,
	onFluxResourceSelect,
}: {
	clusterContext: string;
	selectedKindLabel: string | null;
	selectedFluxResource: FluxResourceSummary | null;
	onFluxResourceSelect: (resource: FluxResourceSummary) => void;
}) {
	const client = useMemo(() => createTauriClient(), []);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	const {
		data: detection,
		isPending: detectionIsPending,
		isError: detectionIsError,
		error: detectionError,
	} = useQuery({
		queryKey: queryKeys.fluxDetect(clusterContext, kubeconfigEnvVar),
		queryFn: () => detectFlux(client, clusterContext, kubeconfigEnvVar),
		enabled: !!clusterContext,
		staleTime: 60_000,
	});
	const resourceKind = fluxKindFromLabel(
		selectedKindLabel,
		detection?.kinds ?? [],
	);
	const {
		data: resources,
		isPending: resourcesIsPending,
		isError: resourcesIsError,
		error: resourcesError,
	} = useQuery({
		queryKey: resourceKind
			? queryKeys.fluxResources(clusterContext, resourceKind, kubeconfigEnvVar)
			: ["flux-resources", clusterContext, selectedKindLabel ?? ""],
		queryFn: () =>
			resourceKind
				? listFluxResources(client, clusterContext, resourceKind, kubeconfigEnvVar)
				: Promise.resolve([]),
		enabled: !!clusterContext && !!resourceKind,
		staleTime: 30_000,
	});

	if (detectionIsPending) return <LoadingState label="Checking for Flux..." />;
	if (detectionIsError) {
		return <ErrorState title="Failed to detect Flux" error={detectionError} />;
	}
	if (!detection?.detected) {
		return (
			<EmptyState
				title="Flux not detected"
				description="This cluster does not currently expose Flux resources."
			/>
		);
	}
	if (!resourceKind) {
		return (
			<EmptyState
				title="Flux resource kind not detected"
				description="The selected Flux CRD is not installed in this cluster."
			/>
		);
	}
	if (resourcesIsPending) return <LoadingState label={`Loading ${resourceKind.kind}...`} />;
	if (resourcesIsError) {
		return <ErrorState title={`Failed to load ${resourceKind.kind}`} error={resourcesError} />;
	}
	const rows = resources ?? [];
	if (rows.length === 0) {
		return <EmptyState title={`No ${resourceKind.kind} resources`} />;
	}

	return (
		<Table className={TABLE_CLASS}>
			<TableHeader>
				<TableRow>
					<TableHead className="w-[22%]">Name</TableHead>
					<TableHead className="w-[14%]">Namespace</TableHead>
					<TableHead className="w-[12%]">Ready</TableHead>
					<TableHead className="w-[16%]">Source</TableHead>
					<TableHead className="w-[16%]">Revision</TableHead>
					<TableHead className="w-[10%]">Age</TableHead>
					<TableHead className="w-[10%]">Inventory</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				{rows.map((row) => {
					const selected =
						selectedFluxResource?.name === row.name &&
						selectedFluxResource?.namespace === row.namespace &&
						selectedFluxResource?.resourceKind.kind === row.resourceKind.kind;
					return (
						<TableRow
							key={`${row.resourceKind.kind}:${row.namespace ?? "_cluster"}:${row.name}`}
							className={cn(ROW_CLASS, selected && SELECTED_ROW_CLASS)}
							tabIndex={0}
							onClick={() => onFluxResourceSelect(row)}
							onKeyDown={(event) => {
								if (event.key !== "Enter" && event.key !== " ") return;
								event.preventDefault();
								onFluxResourceSelect(row);
							}}
						>
							<TableCell title={row.name}>{row.name}</TableCell>
							<TableCell title={row.namespace ?? ""}>{row.namespace ?? "-"}</TableCell>
							<TableCell>
								{row.readyStatus ? (
									<StatusBadge tone={fluxStatusTone(row.readyStatus)}>
										{row.readyStatus}
									</StatusBadge>
								) : (
									"-"
								)}
							</TableCell>
							<TableCell title={sourceLabel(row)}>{sourceLabel(row)}</TableCell>
							<TableCell title={row.lastAppliedRevision ?? ""}>
								{row.lastAppliedRevision ?? "-"}
							</TableCell>
							<TableCell>
								<TimestampText relative={row.age} exact={row.createdAt} />
							</TableCell>
							<TableCell>{row.inventory.length}</TableCell>
						</TableRow>
					);
				})}
			</TableBody>
		</Table>
	);
}

function sourceLabel(row: FluxResourceSummary): string {
	if (!row.sourceKind || !row.sourceName) return "-";
	return `${row.sourceKind}/${row.sourceName}`;
}

function LoadingState({ label }: { label: string }) {
	return (
		<div className={STATE_CLASS}>
			<span className="inline-flex items-center gap-2">
				<Spinner className="size-4" />
				{label}
			</span>
		</div>
	);
}

function EmptyState({
	title,
	description,
}: {
	title: string;
	description?: string;
}) {
	return (
		<Empty className="min-h-64 border-0">
			<EmptyHeader>
				<EmptyTitle>{title}</EmptyTitle>
				{description && <EmptyDescription>{description}</EmptyDescription>}
			</EmptyHeader>
		</Empty>
	);
}

function ErrorState({ title, error }: { title: string; error: unknown }) {
	return (
		<div className="p-4">
			<Alert variant="destructive">
				<AlertTitle>{title}</AlertTitle>
				<AlertDescription>
					{error instanceof Error ? error.message : "Flux request failed"}
				</AlertDescription>
			</Alert>
		</div>
	);
}
