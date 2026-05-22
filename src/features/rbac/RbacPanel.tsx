import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import {
	Alert,
	AlertDescription,
	AlertTitle,
} from "@/components/ui/alert";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Spinner } from "@/components/ui/spinner";
import { queryKeys } from "@/lib/queryKeys";
import { createTauriClient, listRbacInspection } from "@/lib/tauri";
import type { RbacInspectionSummary } from "@/lib/types";
import { collectInspectionRisks, riskyCount } from "./risk";
import {
	BindingsTable,
	NamespaceAccessTable,
	RolesTable,
	ServiceAccountsTable,
} from "./RbacTables";

interface RbacPanelProps {
	clusterContext: string;
	selectedNamespaces: string[];
	selectedView: string | null;
}

const STATE_CLASS =
	"flex min-h-64 items-center justify-center p-8 text-center text-sm text-muted-foreground";

function LoadingState() {
	return (
		<div className={STATE_CLASS}>
			<span className="inline-flex items-center gap-2">
				<Spinner className="size-4" />
				Loading RBAC inspection...
			</span>
		</div>
	);
}

function ErrorState({ error }: { error: unknown }) {
	return (
		<div className="p-4">
			<Alert variant="destructive">
				<AlertTitle>Failed to load RBAC inspection</AlertTitle>
				<AlertDescription>
					{error instanceof Error ? error.message : "Failed to load RBAC inspection"}
				</AlertDescription>
			</Alert>
		</div>
	);
}

function EmptyState() {
	return (
		<Empty className="min-h-64 border-0">
			<EmptyHeader>
				<EmptyTitle>No RBAC resources found</EmptyTitle>
				<EmptyDescription>
					This scope does not currently expose RBAC resources.
				</EmptyDescription>
			</EmptyHeader>
		</Empty>
	);
}

function SummaryCards({ data }: { data: RbacInspectionSummary }) {
	const allRisks = collectInspectionRisks(data);
	const stats = [
		{ label: "Service Accounts", value: data.serviceAccounts.length },
		{ label: "Roles", value: data.roles.length },
		{ label: "Cluster Roles", value: data.clusterRoles.length },
		{ label: "Bindings", value: data.roleBindings.length + data.clusterRoleBindings.length },
		{ label: "Risk Flags", value: allRisks.length },
		{
			label: "Flagged Objects",
			value:
				riskyCount(data.serviceAccounts) +
				riskyCount(data.roles) +
				riskyCount(data.clusterRoles) +
				riskyCount(data.roleBindings) +
				riskyCount(data.clusterRoleBindings),
		},
	];

	return (
		<div className="grid gap-3 md:grid-cols-3 xl:grid-cols-6">
			{stats.map((stat) => (
				<Card key={stat.label} className="rounded-md">
					<CardHeader className="pb-2">
						<CardTitle className="text-xs font-medium uppercase text-muted-foreground">
							{stat.label}
						</CardTitle>
					</CardHeader>
					<CardContent className="pt-0 text-2xl font-semibold">
						{stat.value}
					</CardContent>
				</Card>
			))}
		</div>
	);
}

function SelectedTable({
	data,
	selectedView,
}: {
	data: RbacInspectionSummary;
	selectedView: string | null;
}) {
	const view = selectedView ?? "Namespace Access";
	if (view === "Roles") return <RolesTable rows={data.roles} />;
	if (view === "Cluster Roles") return <RolesTable rows={data.clusterRoles} />;
	if (view === "Bindings") {
		return (
			<BindingsTable
				rows={[...data.roleBindings, ...data.clusterRoleBindings]}
			/>
		);
	}
	if (view === "Service Accounts") {
		return <ServiceAccountsTable rows={data.serviceAccounts} />;
	}
	return <NamespaceAccessTable rows={data.namespaceAccess} />;
}

export function RbacPanel({
	clusterContext,
	selectedNamespaces,
	selectedView,
}: RbacPanelProps) {
	const client = useMemo(() => createTauriClient(), []);
	const {
		data,
		isPending,
		isError,
		error,
	} = useQuery({
		queryKey: queryKeys.rbacInspection(clusterContext, selectedNamespaces),
		queryFn: () => listRbacInspection(client, clusterContext, selectedNamespaces),
		enabled: !!clusterContext,
		staleTime: 30_000,
	});

	if (!clusterContext) {
		return <div className={STATE_CLASS}>Select a cluster context first.</div>;
	}
	if (isPending) return <LoadingState />;
	if (isError) return <ErrorState error={error} />;
	if (!data) return <LoadingState />;
	if (
		data.serviceAccounts.length === 0 &&
		data.roles.length === 0 &&
		data.clusterRoles.length === 0 &&
		data.roleBindings.length === 0 &&
		data.clusterRoleBindings.length === 0
	) {
		return <EmptyState />;
	}

	return (
		<div className="flex min-w-0 flex-col gap-4">
			<SummaryCards data={data} />
			<div className="min-w-0 overflow-x-auto">
				<SelectedTable data={data} selectedView={selectedView} />
			</div>
		</div>
	);
}
