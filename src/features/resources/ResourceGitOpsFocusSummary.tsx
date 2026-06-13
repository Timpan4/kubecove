import { GitBranch } from "lucide-react";
import { TimestampText } from "@/components/TimestampText";
import { StatusChip } from "@/features/argo/StatusChip";
import { healthStatusVariant, syncStatusVariant } from "@/features/argo/status";
import type { ArgoApplicationSummary } from "@/lib/types";

export interface ResourceGitOpsFocus {
	provider: "argo";
	application: ArgoApplicationSummary;
}

function compactNamespaceList(values: string[]): string {
	if (values.length === 0) return "All namespaces";
	if (values.length <= 3) return values.join(", ");
	return `${values.slice(0, 3).join(", ")} +${values.length - 3}`;
}

function SummaryField({
	label,
	value,
	title,
}: {
	label: string;
	value: string | number | null | undefined;
	title?: string;
}) {
	return (
		<div className="min-w-0">
			<div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
				{label}
			</div>
			<div className="min-w-0 truncate text-sm text-foreground" title={title}>
				{value ?? "-"}
			</div>
		</div>
	);
}

export function ResourceGitOpsFocusSummary({
	focus,
}: {
	focus: ResourceGitOpsFocus;
}) {
	const app = focus.application;
	const destination = app.destinationNamespace ?? app.destinationServer;
	const namespaces = app.resourceNamespaces;
	return (
		<section className="rounded-md border border-sidebar-border bg-card/35 px-4 py-3">
			<div className="flex flex-wrap items-start justify-between gap-3">
				<div className="min-w-0">
					<div className="flex min-w-0 items-center gap-2">
						<GitBranch className="size-4 shrink-0 text-[var(--resource-argo)]" />
						<h2 className="truncate text-base font-semibold">{app.name}</h2>
					</div>
					<div className="mt-2 flex flex-wrap gap-1.5">
						<StatusChip
							value={app.healthStatus}
							variant={healthStatusVariant(app.healthStatus)}
						/>
						<StatusChip
							value={app.syncStatus}
							variant={syncStatusVariant(app.syncStatus)}
						/>
					</div>
				</div>
				<div className="text-right text-sm">
					<div className="font-semibold text-foreground">
						{app.trackedResourceCount ?? "-"}
					</div>
					<div className="text-xs text-muted-foreground">tracked resources</div>
				</div>
			</div>
			<div className="mt-3 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
				<SummaryField label="Project" value={app.project} />
				<SummaryField
					label="Repo"
					value={app.sourceRepo}
					title={app.sourceRepo ?? undefined}
				/>
				<SummaryField label="Revision" value={app.sourceRevision} />
				<SummaryField label="Destination" value={destination} />
				<SummaryField label="App namespace" value={app.namespace} />
				<SummaryField
					label="Resource namespaces"
					value={compactNamespaceList(namespaces)}
					title={namespaces.length > 0 ? namespaces.join(", ") : undefined}
				/>
				<div className="min-w-0">
					<div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
						Created
					</div>
					<div className="text-sm text-foreground">
						<TimestampText relative={app.age} exact={app.createdAt} />
					</div>
				</div>
			</div>
		</section>
	);
}
