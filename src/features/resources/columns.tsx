import { createColumnHelper } from "@tanstack/react-table";
import type { ResourceSummary } from "@/lib/types";
import {
	AgeCell,
	ArgoHelmBadges,
	KindCell,
	RestartsCell,
	StatusChip,
	TruncatedCell,
	type ChipVariant,
} from "./cells";

const columnHelper = createColumnHelper<ResourceSummary>();

export const columns = [
	columnHelper.accessor("name", {
		header: "Name",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.accessor("namespace", {
		header: "Namespace",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.accessor("kind", {
		header: "Kind",
		cell: (info) => <KindCell kind={info.getValue()} />,
	}),
	columnHelper.accessor("status", {
		header: "Status",
		cell: (info) => {
			const value = info.getValue();
			if (!value) return "—";
			const variant: ChipVariant =
				value === "Running" || value === "Succeeded" || value === "Ready"
					? "success"
					: value === "Pending" || value === "Terminating"
						? "warning"
						: value === "Failed" || value === "Error"
							? "error"
							: "neutral";
			return <StatusChip value={value} variant={variant} />;
		},
	}),
	columnHelper.accessor("ready", {
		header: "Ready",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.accessor("restarts", {
		header: () => <span className="block text-center">Restarts</span>,
		cell: (info) => <RestartsCell value={info.getValue()} />,
	}),
	columnHelper.accessor("ownerRef", {
		header: "Owner",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.accessor("age", {
		header: "Age",
		cell: (info) => <AgeCell row={info.row.original} />,
	}),
	columnHelper.display({
		id: "argo-helm",
		header: "App",
		cell: ({ row }) => <ArgoHelmBadges row={row.original} />,
		enableSorting: false,
	}),
];
