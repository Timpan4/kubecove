import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	FluxResourceSummary,
	VersionedList,
} from "@/lib/types";
import type { GitOpsData } from "./surfaceModel";

type DetectionQuery<T> = {
	data?: T;
	isSuccess: boolean;
	isError: boolean;
	isPending: boolean;
	error: unknown;
};

type ListQuery<T> = {
	data?: VersionedList<T>;
	isPending: boolean;
	error: unknown;
};

export interface GitOpsQuerySnapshots {
	argoDetection: DetectionQuery<boolean>;
	fluxDetection: DetectionQuery<{ detected: boolean }>;
	argoApps: ListQuery<ArgoApplicationSummary>;
	argoAppSets: ListQuery<ArgoApplicationSetSummary>;
	argoProjects: ListQuery<ArgoAppProjectSummary>;
	fluxResources: Array<ListQuery<FluxResourceSummary>>;
}

export function buildGitOpsReadState(snapshots: GitOpsQuerySnapshots) {
	const {
		argoDetection,
		fluxDetection,
		argoApps,
		argoAppSets,
		argoProjects,
		fluxResources,
	} = snapshots;
	// Read every pending flag before branching so query observers track each
	// transition, including lists that settle while detection is still pending.
	const argoPending = argoDetection.isPending;
	const fluxPending = fluxDetection.isPending;
	const appListsPending = [argoApps.isPending, argoAppSets.isPending, argoProjects.isPending].some(Boolean);
	const fluxListsPending = fluxResources.map(query => query.isPending).some(Boolean);
	const detectionReady =
		(argoDetection.isSuccess || argoDetection.isError) &&
		(fluxDetection.isSuccess || fluxDetection.isError);
	const data: GitOpsData | undefined = detectionReady
		? {
				argoDetected: argoDetection.data === true,
				apps: argoApps.data?.items ?? [],
				appSets: argoAppSets.data?.items ?? [],
				projects: argoProjects.data?.items ?? [],
				flux: fluxResources.flatMap((query) => query.data?.items ?? []),
				fluxDetected: fluxDetection.data?.detected === true,
			}
		: undefined;

	return {
		providerError: argoDetection.error ?? fluxDetection.error,
		listError:
			argoApps.error ??
			argoAppSets.error ??
			argoProjects.error ??
			fluxResources.find((query) => query.error)?.error,
		query: {
			data,
			isPending:
				argoPending || fluxPending ||
				(argoDetection.data === true && appListsPending) ||
				(fluxDetection.data?.detected === true && fluxListsPending),
			isError: false,
			error: null,
		},
	};
}
