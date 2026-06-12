import { createColumnHelper } from "@tanstack/react-table";
import type { ResourceSummary } from "@/lib/types";
import {
	AgeCell,
	ArgoHelmBadges,
	CpuCell,
	MemoryCell,
	RestartsCell,
	StatusChip,
	TruncatedCell,
	type ChipVariant,
} from "./cells";

const columnHelper = createColumnHelper<ResourceSummary>();

const SUCCESS_STATUS_VALUES = new Set([
	"Running",
	"Succeeded",
	"Complete",
	"Completed",
	"Ready",
]);

function isSuccessfulTerminalPod(
	row: Pick<ResourceSummary, "kind" | "status">,
): boolean {
	return (
		row.kind === "Pod" &&
		["succeeded", "complete", "completed"].includes(
			row.status?.trim().toLowerCase() ?? "",
		)
	);
}

export function resourceStatusTone(value: string): ChipVariant {
	return SUCCESS_STATUS_VALUES.has(value)
		? "success"
		: value === "Pending" || value === "Terminating"
			? "warning"
			: value === "Failed" || value === "Error"
				? "error"
				: "neutral";
}

export function resourceReadyChip(
	row: Pick<ResourceSummary, "kind" | "status" | "ready">,
): { value: string; variant: ChipVariant } | null {
	if (row.ready === "True") return { value: "Ready", variant: "success" };
	if (row.ready === "False") {
		return isSuccessfulTerminalPod(row)
			? { value: "Completed", variant: "success" }
			: { value: "Not ready", variant: "error" };
	}
	return null;
}

export const columns = [
	columnHelper.accessor("name", {
		header: "Name",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
		size: 250,
	}),
	columnHelper.accessor("namespace", {
		header: "Namespace",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
		size: 105,
	}),
	columnHelper.accessor("status", {
		header: "Status",
		size: 95,
		cell: (info) => {
			const value = info.getValue();
			if (!value) return "—";
			return <StatusChip value={value} variant={resourceStatusTone(value)} />;
		},
	}),
	columnHelper.accessor("ready", {
		header: "Ready",
		cell: (info) => {
			const value = info.getValue();
			const row = info.row.original;
			// Pods report readiness as a raw boolean string; controllers as n/n.
			const readyChip = resourceReadyChip(row);
			if (readyChip) {
				return (
					<StatusChip
						value={readyChip.value}
						variant={readyChip.variant}
					/>
				);
			}
			return <TruncatedCell value={value} />;
		},
		size: 70,
	}),
	columnHelper.accessor("restarts", {
		header: () => <span className="block text-center">Restarts</span>,
		cell: (info) => <RestartsCell value={info.getValue()} />,
		size: 75,
	}),
	columnHelper.accessor((row) => row.metrics?.cpuMillicores, {
		id: "cpu",
		header: "CPU",
		cell: (info) => <CpuCell value={info.getValue()} />,
		size: 65,
	}),
	columnHelper.accessor((row) => row.metrics?.memoryBytes, {
		id: "memory",
		header: "Memory",
		cell: (info) => <MemoryCell value={info.getValue()} />,
		size: 85,
	}),
	columnHelper.accessor("ownerRef", {
		header: "Owner",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
		size: 110,
	}),
	columnHelper.accessor("age", {
		header: "Age",
		cell: (info) => <AgeCell row={info.row.original} />,
		size: 65,
	}),
	columnHelper.display({
		id: "argo-helm",
		header: "App",
		cell: ({ row }) => <ArgoHelmBadges row={row.original} />,
		enableSorting: false,
		size: 90,
	}),
];
