import { useMemo } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import {
	ArrowRight,
	Bell,
	Boxes,
	GitBranch,
	Image,
	Layers,
	PackageSearch,
} from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
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
import type { FluxResourceKind, FluxResourceSummary } from "@/lib/types";
import {
	ARGO_NAV_KINDS,
	ARGO_PROVIDER_GROUP_ID,
	FLUX_FAMILIES,
	fluxKindDefinitionFromLabel,
} from "./gitops-nav";

interface GitOpsOverviewProps {
	clusterContext: string;
	onGitOpsKindSelect: (kind: string, group?: string) => void;
}

function CountRow({
	label,
	value,
	tone,
}: {
	label: string;
	value: number | string;
	tone?: string;
}) {
	return (
		<div className="flex items-center justify-between gap-3 py-2 text-sm">
			<span className="min-w-0 truncate text-muted-foreground">{label}</span>
			<strong className={tone}>{value}</strong>
		</div>
	);
}

function InlineLoading({ label }: { label: string }) {
	return (
		<div className="inline-flex items-center gap-2 text-sm text-muted-foreground">
			<Spinner className="size-4" />
			{label}
		</div>
	);
}

function ProviderUnavailableCard({ provider }: { provider: "Argo CD" | "Flux" }) {
	return (
		<Card className="opacity-65">
			<CardHeader>
				<CardTitle>{provider}</CardTitle>
				<CardDescription>Not detected in this cluster.</CardDescription>
			</CardHeader>
		</Card>
	);
}

function queryErrorMessage(error: unknown): string {
	return error instanceof Error ? error.message : "GitOps request failed";
}

function matchingFluxKind(
	kinds: FluxResourceKind[],
	kindName: string,
): FluxResourceKind | null {
	return kinds.find((kind) => kind.kind === kindName) ?? null;
}

function familyRows(
	rows: FluxResourceSummary[],
	kinds: FluxResourceKind[],
	familyKey: string,
) {
	const family = FLUX_FAMILIES.find((candidate) => candidate.key === familyKey);
	if (!family) return { count: 0, installed: 0 };
	const kindNames = new Set(family.kinds.map((kind) => kind.kind));
	return {
		count: rows.filter((row) => kindNames.has(row.resourceKind.kind)).length,
		installed: kinds.filter((kind) => kindNames.has(kind.kind)).length,
	};
}

export function GitOpsOverview({
	clusterContext,
	onGitOpsKindSelect,
}: GitOpsOverviewProps) {
	const client = useMemo(() => createTauriClient(), []);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	const showUnavailableGitOpsProviders = useSettingsState(
		(state) => state.showUnavailableGitOpsProviders,
	);

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
	const showArgo =
		argoDetected ||
		argoDetection.isPending ||
		argoDetection.isError ||
		(showUnavailableGitOpsProviders && argoDetection.data === false);
	const showFlux =
		fluxDetected ||
		fluxDetection.isPending ||
		fluxDetection.isError ||
		(showUnavailableGitOpsProviders && fluxDetection.data?.detected === false);

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
	const fluxReady = fluxRows.filter((row) => row.readyStatus === "True").length;
	const fluxAttention = fluxRows.filter(
		(row) => row.readyStatus === "False" || row.suspended === true,
	).length;
	const argoOutOfSync = (apps.data ?? []).filter(
		(app) => app.syncStatus && app.syncStatus !== "Synced",
	).length;
	const argoUnhealthy = (apps.data ?? []).filter(
		(app) => app.healthStatus && app.healthStatus !== "Healthy",
	).length;
	const noProvidersVisible =
		!showArgo &&
		!showFlux &&
		argoDetection.isSuccess &&
		fluxDetection.isSuccess;

	return (
		<div className="mx-auto flex w-full max-w-6xl flex-col gap-4">
			<div className="border-b pb-3">
				<h1 className="text-lg font-semibold">GitOps</h1>
				<p className="mt-1 text-sm text-muted-foreground">
					Read-only provider inventory for this cluster.
				</p>
			</div>

			{noProvidersVisible && (
				<Empty className="min-h-64">
					<EmptyHeader>
						<EmptyTitle>No GitOps providers detected</EmptyTitle>
						<EmptyDescription>
							Enable "Show unavailable GitOps providers" in General settings to
							keep Argo CD and Flux placeholders visible.
						</EmptyDescription>
					</EmptyHeader>
				</Empty>
			)}

			<div className="grid gap-4 lg:grid-cols-2">
				{showArgo && (
					<>
						{argoDetection.isPending ? (
							<Card>
								<CardHeader>
									<CardTitle>Argo CD</CardTitle>
									<CardDescription>
										Checking provider CRDs for this cluster.
									</CardDescription>
								</CardHeader>
								<CardContent>
									<InlineLoading label="Detecting Argo CD..." />
								</CardContent>
							</Card>
						) : argoDetection.isError ? (
							<Card>
								<CardHeader>
									<CardTitle>Argo CD</CardTitle>
									<CardDescription>
										Provider detection did not complete.
									</CardDescription>
								</CardHeader>
								<CardContent>
									<Alert variant="destructive">
										<AlertTitle>Failed to detect Argo CD</AlertTitle>
										<AlertDescription>
											{queryErrorMessage(argoDetection.error)}
										</AlertDescription>
									</Alert>
								</CardContent>
							</Card>
						) : argoDetected ? (
							<Card>
								<CardHeader>
									<div className="flex items-start justify-between gap-3">
										<div>
											<CardTitle>Argo CD</CardTitle>
											<CardDescription>
												Applications, ApplicationSets, and AppProjects.
											</CardDescription>
										</div>
										<Badge variant="outline" className="rounded-sm">
											Detected
										</Badge>
									</div>
								</CardHeader>
								<CardContent className="grid gap-3">
									{apps.isPending || appsets.isPending || projects.isPending ? (
										<InlineLoading label="Loading Argo CD inventory..." />
									) : apps.isError || appsets.isError || projects.isError ? (
										<Alert variant="destructive">
											<AlertTitle>Argo CD counts unavailable</AlertTitle>
											<AlertDescription>
												{queryErrorMessage(
													apps.error ?? appsets.error ?? projects.error,
												)}
											</AlertDescription>
										</Alert>
									) : (
										<>
											<CountRow
												label="Argo CD Applications"
												value={apps.data?.length ?? 0}
											/>
											<Separator />
											<CountRow
												label="Argo CD ApplicationSets"
												value={appsets.data?.length ?? 0}
											/>
											<Separator />
											<CountRow
												label="Argo CD AppProjects"
												value={projects.data?.length ?? 0}
											/>
											<Separator />
											<CountRow
												label="Out of sync"
												value={argoOutOfSync}
												tone="text-amber-300"
											/>
											<CountRow
												label="Unhealthy"
												value={argoUnhealthy}
												tone="text-red-300"
											/>
										</>
									)}
									<div className="flex flex-wrap gap-2 pt-1">
										{ARGO_NAV_KINDS.map((kind) => (
											<Button
												key={kind.key}
												type="button"
												variant="outline"
												size="sm"
												onClick={() =>
													onGitOpsKindSelect(kind.label, ARGO_PROVIDER_GROUP_ID)
												}
											>
												<GitBranch data-icon="inline-start" />
												{kind.label.replace("Argo CD ", "")}
												<ArrowRight data-icon="inline-end" />
											</Button>
										))}
									</div>
								</CardContent>
							</Card>
						) : (
							<ProviderUnavailableCard provider="Argo CD" />
						)}
					</>
				)}

				{showFlux && (
					<>
						{fluxDetection.isPending ? (
							<Card>
								<CardHeader>
									<CardTitle>Flux</CardTitle>
									<CardDescription>
										Checking provider CRDs for this cluster.
									</CardDescription>
								</CardHeader>
								<CardContent>
									<InlineLoading label="Detecting Flux..." />
								</CardContent>
							</Card>
						) : fluxDetection.isError ? (
							<Card>
								<CardHeader>
									<CardTitle>Flux</CardTitle>
									<CardDescription>
										Provider detection did not complete.
									</CardDescription>
								</CardHeader>
								<CardContent>
									<Alert variant="destructive">
										<AlertTitle>Failed to detect Flux</AlertTitle>
										<AlertDescription>
											{queryErrorMessage(fluxDetection.error)}
										</AlertDescription>
									</Alert>
								</CardContent>
							</Card>
						) : fluxDetected ? (
							<Card>
								<CardHeader>
									<div className="flex items-start justify-between gap-3">
										<div>
											<CardTitle>Flux</CardTitle>
											<CardDescription>
												Sources, workloads, notifications, and image automation.
											</CardDescription>
										</div>
										<Badge variant="outline" className="rounded-sm">
											{fluxKinds.length} kinds
										</Badge>
									</div>
								</CardHeader>
								<CardContent className="grid gap-3">
									{fluxPending ? (
										<InlineLoading label="Loading Flux inventory..." />
									) : fluxError ? (
										<Alert variant="destructive">
											<AlertTitle>Flux counts unavailable</AlertTitle>
											<AlertDescription>
												{queryErrorMessage(fluxError)}
											</AlertDescription>
										</Alert>
									) : (
										<>
											{FLUX_FAMILIES.map((family) => {
												const summary = familyRows(
													fluxRows,
													fluxKinds,
													family.key,
												);
												const Icon =
													family.key === "sources"
														? PackageSearch
														: family.key === "workloads"
															? Layers
															: family.key === "notifications"
																? Bell
																: Image;
												return (
													<div key={family.key}>
														<div className="flex items-center justify-between gap-3 py-2 text-sm">
															<span className="inline-flex min-w-0 items-center gap-2 truncate text-muted-foreground">
																<Icon className="size-4 shrink-0" />
																{family.label}
															</span>
															<strong>
																{summary.count} resources / {summary.installed} kinds
															</strong>
														</div>
														<Separator />
													</div>
												);
											})}
											<CountRow
												label="Ready"
												value={fluxReady}
												tone="text-emerald-300"
											/>
											<CountRow
												label="Needs attention"
												value={fluxAttention}
												tone="text-amber-300"
											/>
										</>
									)}
									<div className="flex flex-wrap gap-2 pt-1">
										{FLUX_FAMILIES.map((family) => (
											<Button
												key={family.key}
												type="button"
												variant="outline"
												size="sm"
												disabled={
													!family.kinds.some((kind) =>
														matchingFluxKind(fluxKinds, kind.kind),
													)
												}
												onClick={() => {
													const firstInstalled = family.kinds.find((kind) =>
														matchingFluxKind(fluxKinds, kind.kind),
													);
													const definition = fluxKindDefinitionFromLabel(
														firstInstalled?.label ?? null,
													);
													if (!definition) return;
													onGitOpsKindSelect(
														definition.label,
														family.groupId,
													);
												}}
											>
												<Boxes data-icon="inline-start" />
												{family.label}
												<ArrowRight data-icon="inline-end" />
											</Button>
										))}
									</div>
								</CardContent>
							</Card>
						) : (
							<ProviderUnavailableCard provider="Flux" />
						)}
					</>
				)}
			</div>
		</div>
	);
}
