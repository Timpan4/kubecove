import { useEffect, useMemo, useState } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
import { queryKeys } from "@/lib/queryKeys";
import { useSettingsState } from "@/lib/settings";
import {
	createTauriClient,
	detectArgoCD,
	detectFlux,
	listArgoApplications,
	listArgoApplicationSets,
	listArgoAppProjects,
	listFluxResources,
} from "@/lib/tauri";
import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	FluxResourceSummary,
} from "@/lib/types";
import { cn } from "@/lib/utils";
import {
	buildGitOpsOverviewFilters,
	chooseDefaultGitOpsFilter,
	type GitOpsOverviewFilter,
	type GitOpsOverviewFilterKey,
} from "./gitops-overview-helpers";
import { GitOpsCardGrid, LoadingState } from "./GitOpsOverviewCards";

type ArgoSummaryItem =
	| ArgoApplicationSummary
	| ArgoApplicationSetSummary
	| ArgoAppProjectSummary;

interface GitOpsOverviewProps {
	clusterContext: string;
	selectedGitOpsItem: ArgoSummaryItem | null;
	selectedFluxResource: FluxResourceSummary | null;
	onGitOpsItemSelect: (item: ArgoSummaryItem) => void;
	onFluxResourceSelect: (resource: FluxResourceSummary) => void;
}

const EMPTY_APPS: ArgoApplicationSummary[] = [];
const EMPTY_APPSETS: ArgoApplicationSetSummary[] = [];
const EMPTY_PROJECTS: ArgoAppProjectSummary[] = [];

function queryErrorMessage(error: unknown): string {
	return error instanceof Error ? error.message : "GitOps request failed";
}

function shortLabel(filter: GitOpsOverviewFilter): string {
	if (filter.provider === "Argo CD") return filter.label.replace("Argo CD ", "");
	return filter.label.replace("Flux ", "");
}

function FilterRail({
	filters,
	activeKey,
	onSelect,
}: {
	filters: GitOpsOverviewFilter[];
	activeKey: GitOpsOverviewFilterKey | null;
	onSelect: (key: GitOpsOverviewFilterKey) => void;
}) {
	const providers = ["Argo CD", "Flux"] as const;
	return (
		<aside className="w-full shrink-0 lg:w-64">
			<div className="sticky top-0 flex flex-col gap-4">
				{providers.map((provider) => {
					const providerFilters = filters.filter(
						(filter) => filter.provider === provider,
					);
					if (providerFilters.length === 0) return null;
					const groups = [...new Set(providerFilters.map((filter) => filter.group))];
					return (
						<section key={provider} className="space-y-2">
							<div className="px-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
								{provider}
							</div>
							{groups.map((group) => {
								const groupFilters = providerFilters.filter(
									(filter) => filter.group === group,
								);
								return (
									<div key={group} className="space-y-1">
										{provider === "Flux" && (
											<div className="px-1 py-1 text-xs text-muted-foreground">
												{group}
												{groupFilters[0]?.installedKinds !== undefined &&
													` · ${groupFilters[0].installedKinds} kinds`}
											</div>
										)}
										{groupFilters.map((filter) => (
											<button
												key={filter.key}
												type="button"
												className={cn(
													"flex h-8 w-full cursor-pointer items-center gap-2 rounded-md px-2.5 text-left text-sm transition-colors focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-ring/50",
													activeKey === filter.key
														? "bg-secondary font-medium text-foreground"
														: "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
													filter.disabled &&
														"cursor-default opacity-50 hover:bg-transparent hover:text-muted-foreground",
												)}
												disabled={filter.disabled}
												onClick={() => onSelect(filter.key)}
											>
												<span className="min-w-0 flex-1 truncate">
													{shortLabel(filter)}
												</span>
												<Badge variant="secondary" className="rounded-sm px-1.5">
													{filter.count}
												</Badge>
											</button>
										))}
									</div>
								);
							})}
						</section>
					);
				})}
			</div>
		</aside>
	);
}

export function GitOpsOverview({
	clusterContext,
	selectedGitOpsItem,
	selectedFluxResource,
	onGitOpsItemSelect,
	onFluxResourceSelect,
}: GitOpsOverviewProps) {
	const client = useMemo(() => createTauriClient(), []);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	const showUnavailableGitOpsProviders = useSettingsState(
		(state) => state.showUnavailableGitOpsProviders,
	);
	const [selectedFilter, setSelectedFilter] =
		useState<GitOpsOverviewFilterKey | null>(null);
	const [userSelectedFilter, setUserSelectedFilter] = useState(false);

	const argoDetection = useQuery({
		queryKey: queryKeys.argoDetect(clusterContext, kubeconfigEnvVar),
		queryFn: () => detectArgoCD(client, clusterContext, kubeconfigEnvVar),
		enabled: !!clusterContext,
		staleTime: 60_000,
	});
	const fluxDetection = useQuery({
		queryKey: queryKeys.fluxDetect(clusterContext, kubeconfigEnvVar),
		queryFn: () => detectFlux(client, clusterContext, kubeconfigEnvVar),
		enabled: !!clusterContext,
		staleTime: 60_000,
	});
	const argoDetected = argoDetection.data === true;
	const fluxDetected = fluxDetection.data?.detected === true;

	const apps = useQuery({
		queryKey: queryKeys.argoApps(clusterContext, kubeconfigEnvVar),
		queryFn: () => listArgoApplications(client, clusterContext, kubeconfigEnvVar),
		enabled: !!clusterContext && argoDetected,
		staleTime: 30_000,
	});
	const appsets = useQuery({
		queryKey: queryKeys.argoAppSets(clusterContext, kubeconfigEnvVar),
		queryFn: () =>
			listArgoApplicationSets(client, clusterContext, kubeconfigEnvVar),
		enabled: !!clusterContext && argoDetected,
		staleTime: 30_000,
	});
	const projects = useQuery({
		queryKey: queryKeys.argoAppProjects(clusterContext, kubeconfigEnvVar),
		queryFn: () => listArgoAppProjects(client, clusterContext, kubeconfigEnvVar),
		enabled: !!clusterContext && argoDetected,
		staleTime: 30_000,
	});

	const fluxKinds = fluxDetection.data?.kinds ?? [];
	const fluxResourceQueries = useQueries({
		queries: fluxKinds.map((resourceKind) => ({
			queryKey: queryKeys.fluxResources(
				clusterContext,
				resourceKind,
				kubeconfigEnvVar,
			),
			queryFn: () =>
				listFluxResources(client, clusterContext, resourceKind, kubeconfigEnvVar),
			enabled: !!clusterContext && fluxDetected,
			staleTime: 30_000,
		})),
	});
	const fluxRows = fluxResourceQueries.flatMap((query) => query.data ?? []);
	const fluxError = fluxResourceQueries.find((query) => query.isError)?.error;
	const fluxPending =
		fluxResourceQueries.length > 0 &&
		fluxResourceQueries.some((query) => query.isPending);

	const filters = useMemo(
		() =>
			buildGitOpsOverviewFilters({
				argoDetected,
				showUnavailableArgo:
					showUnavailableGitOpsProviders && argoDetection.data === false,
				apps: apps.data ?? EMPTY_APPS,
				appsets: appsets.data ?? EMPTY_APPSETS,
				projects: projects.data ?? EMPTY_PROJECTS,
				fluxDetected,
				showUnavailableFlux:
					showUnavailableGitOpsProviders &&
					fluxDetection.data?.detected === false,
				fluxKinds,
				fluxRows,
			}),
		[
			argoDetected,
			argoDetection.data,
			apps.data,
			appsets.data,
			projects.data,
			fluxDetected,
			fluxDetection.data?.detected,
			fluxKinds,
			fluxRows,
			showUnavailableGitOpsProviders,
		],
	);
	const defaultFilter = useMemo(
		() => chooseDefaultGitOpsFilter(filters),
		[filters],
	);
	const activeFilterKey = selectedFilter ?? defaultFilter;
	const activeFilter =
		filters.find((filter) => filter.key === activeFilterKey) ?? null;

	useEffect(() => {
		if (!defaultFilter) {
			setSelectedFilter(null);
			setUserSelectedFilter(false);
			return;
		}
		const selectedFilterIsValid =
			selectedFilter &&
			filters.some((filter) => filter.key === selectedFilter && !filter.disabled);
		if (selectedFilterIsValid && userSelectedFilter) {
			return;
		}
		setSelectedFilter(defaultFilter);
	}, [defaultFilter, filters, selectedFilter, userSelectedFilter]);

	function handleFilterSelect(filterKey: GitOpsOverviewFilterKey) {
		setUserSelectedFilter(true);
		setSelectedFilter(filterKey);
	}

	const initialLoading = argoDetection.isPending || fluxDetection.isPending;
	const noProvidersVisible =
		filters.length === 0 && argoDetection.isSuccess && fluxDetection.isSuccess;
	const hasListError =
		apps.isError || appsets.isError || projects.isError || Boolean(fluxError);

	return (
		<div className="mx-auto flex w-full max-w-7xl flex-col gap-4">
			<div className="border-b pb-3">
				<h1 className="text-lg font-semibold">GitOps</h1>
				<p className="mt-1 text-sm text-muted-foreground">
					Read-only resource cards for detected GitOps providers.
				</p>
			</div>

			{argoDetection.isError && (
				<Alert variant="destructive">
					<AlertTitle>Failed to detect Argo CD</AlertTitle>
					<AlertDescription>{queryErrorMessage(argoDetection.error)}</AlertDescription>
				</Alert>
			)}
			{fluxDetection.isError && (
				<Alert variant="destructive">
					<AlertTitle>Failed to detect Flux</AlertTitle>
					<AlertDescription>{queryErrorMessage(fluxDetection.error)}</AlertDescription>
				</Alert>
			)}
			{hasListError && (
				<Alert variant="destructive">
					<AlertTitle>Some GitOps resources could not load</AlertTitle>
					<AlertDescription>
						{queryErrorMessage(
							apps.error ?? appsets.error ?? projects.error ?? fluxError,
						)}
					</AlertDescription>
				</Alert>
			)}

			{initialLoading && filters.length === 0 ? (
				<LoadingState label="Checking GitOps providers..." />
			) : noProvidersVisible ? (
				<Empty className="min-h-64">
					<EmptyHeader>
						<EmptyTitle>No GitOps providers detected</EmptyTitle>
						<EmptyDescription>
							Enable "Show unavailable GitOps providers" in General settings to
							keep Argo CD and Flux placeholders visible.
						</EmptyDescription>
					</EmptyHeader>
				</Empty>
			) : (
				<div className="grid gap-6 lg:grid-cols-[260px_minmax(0,1fr)]">
					<FilterRail
						filters={filters}
						activeKey={activeFilterKey}
						onSelect={handleFilterSelect}
					/>
					<section className="min-w-0">
						<div className="mb-3 flex flex-wrap items-center justify-between gap-3">
							<div className="min-w-0">
								<h2 className="truncate text-base font-semibold">
									{activeFilter ? shortLabel(activeFilter) : "GitOps resources"}
								</h2>
								<p className="text-sm text-muted-foreground">
									{activeFilter?.count ?? 0} resources
								</p>
							</div>
						</div>
						{fluxPending ? (
							<LoadingState label="Loading Flux resources..." />
						) : (
							<GitOpsCardGrid
								activeFilter={activeFilter}
								apps={apps.data ?? EMPTY_APPS}
								appsets={appsets.data ?? EMPTY_APPSETS}
								projects={projects.data ?? EMPTY_PROJECTS}
								fluxRows={fluxRows}
								selectedGitOpsItem={selectedGitOpsItem}
								selectedFluxResource={selectedFluxResource}
								onGitOpsItemSelect={onGitOpsItemSelect}
								onFluxResourceSelect={onFluxResourceSelect}
							/>
						)}
					</section>
				</div>
			)}
		</div>
	);
}
