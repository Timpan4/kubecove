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
import { Spinner } from "@/components/ui/spinner";
import {
	createTauriClient,
	detectArgoCD,
	listArgoApplications,
	listArgoApplicationSets,
	listArgoAppProjects,
} from "@/lib/tauri";
import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
} from "@/lib/types";
import {
	ApplicationSetsTable,
	ApplicationsTable,
	AppProjectsTable,
} from "./ArgoTables";

type ArgoSummaryItem =
	| ArgoApplicationSummary
	| ArgoApplicationSetSummary
	| ArgoAppProjectSummary;

interface ArgoCDPanelProps {
	clusterContext: string;
	selectedArgoItem: ArgoSummaryItem | null;
	onArgoItemSelect: (item: ArgoSummaryItem) => void;
	selectedArgoKind: string | null;
}

const STATE_CLASS =
	"flex min-h-64 items-center justify-center p-8 text-center text-sm text-muted-foreground";

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

function ErrorState({
	title,
	error,
	fallback,
}: {
	title: string;
	error: unknown;
	fallback: string;
}) {
	return (
		<div className="p-4">
			<Alert variant="destructive">
				<AlertTitle>{title}</AlertTitle>
				<AlertDescription>
					{error instanceof Error ? error.message : fallback}
				</AlertDescription>
			</Alert>
		</div>
	);
}

export function ArgoCDPanel({
	clusterContext,
	selectedArgoItem,
	onArgoItemSelect,
	selectedArgoKind,
}: ArgoCDPanelProps) {
	const client = useMemo(() => createTauriClient(), []);

	const { data: argoDetected, isPending: detectPending } = useQuery({
		queryKey: ["argo-detect", clusterContext],
		queryFn: () => detectArgoCD(client, clusterContext),
		enabled: !!clusterContext,
		staleTime: 60_000,
	});

	const isApps = selectedArgoKind === "Applications";
	const isAppSets = selectedArgoKind === "ApplicationSets";
	const isAppProjects = selectedArgoKind === "AppProjects";

	const {
		data: apps,
		isPending: appsPending,
		isError: appsError,
		error: appsErr,
	} = useQuery({
		queryKey: ["argo-apps", clusterContext],
		queryFn: () => listArgoApplications(client, clusterContext),
		enabled: !!clusterContext && argoDetected === true && isApps,
		staleTime: 30_000,
	});

	const {
		data: appsets,
		isPending: appsetsPending,
		isError: appsetsError,
		error: appsetsErr,
	} = useQuery({
		queryKey: ["argo-appsets", clusterContext],
		queryFn: () => listArgoApplicationSets(client, clusterContext),
		enabled: !!clusterContext && argoDetected === true && isAppSets,
		staleTime: 30_000,
	});

	const {
		data: projects,
		isPending: projectsPending,
		isError: projectsError,
		error: projectsErr,
	} = useQuery({
		queryKey: ["argo-appprojects", clusterContext],
		queryFn: () => listArgoAppProjects(client, clusterContext),
		enabled: !!clusterContext && argoDetected === true && isAppProjects,
		staleTime: 30_000,
	});

	if (detectPending) {
		return <LoadingState label="Checking for Argo CD..." />;
	}

	if (argoDetected === false) {
		return (
			<EmptyState
				title="Argo CD not detected"
				description="This cluster does not currently expose Argo CD resources."
			/>
		);
	}

	if (!selectedArgoKind) {
		return (
			<EmptyState
				title="Select an Argo CD resource type"
				description="Choose applications, application sets, or app projects from the tree."
			/>
		);
	}

	if (isApps) {
		if (appsPending) {
			return <LoadingState label="Loading Argo CD applications..." />;
		}
		if (appsError) {
			return (
				<ErrorState
					title="Failed to load applications"
					error={appsErr}
					fallback="Failed to load applications"
				/>
			);
		}
		if (!apps) {
			return <LoadingState label="Loading..." />;
		}
		return (
			<div className="flex flex-col">
				<ApplicationsTable
					apps={apps}
					selectedArgoApp={selectedArgoItem as ArgoApplicationSummary | null}
					onAppSelect={
						onArgoItemSelect as (app: ArgoApplicationSummary) => void
					}
				/>
			</div>
		);
	}

	if (isAppSets) {
		if (appsetsPending) {
			return <LoadingState label="Loading Argo CD application sets..." />;
		}
		if (appsetsError) {
			return (
				<ErrorState
					title="Failed to load application sets"
					error={appsetsErr}
					fallback="Failed to load application sets"
				/>
			);
		}
		if (!appsets) {
			return <LoadingState label="Loading..." />;
		}
		return (
			<div className="flex flex-col">
				<ApplicationSetsTable
					appsets={appsets}
					selectedArgoItem={selectedArgoItem as ArgoApplicationSetSummary | null}
					onArgoItemSelect={
						onArgoItemSelect as (item: ArgoApplicationSetSummary) => void
					}
				/>
			</div>
		);
	}

	if (isAppProjects) {
		if (projectsPending) {
			return <LoadingState label="Loading Argo CD app projects..." />;
		}
		if (projectsError) {
			return (
				<ErrorState
					title="Failed to load app projects"
					error={projectsErr}
					fallback="Failed to load app projects"
				/>
			);
		}
		if (!projects) {
			return <LoadingState label="Loading..." />;
		}
		return (
			<div className="flex flex-col">
				<AppProjectsTable
					projects={projects}
					selectedArgoItem={selectedArgoItem as ArgoAppProjectSummary | null}
					onArgoItemSelect={
						onArgoItemSelect as (item: ArgoAppProjectSummary) => void
					}
				/>
			</div>
		);
	}

	return <EmptyState title={`${selectedArgoKind} is not yet supported`} />;
}
