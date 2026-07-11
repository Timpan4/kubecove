import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	FluxResourceSummary,
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
	data?: T;
	isPending: boolean;
	error: unknown;
};

export interface GitOpsQuerySnapshots {
	argoDetection: DetectionQuery<boolean>;
	fluxDetection: DetectionQuery<{ detected: boolean }>;
	argoApps: ListQuery<ArgoApplicationSummary[]>;
	argoAppSets: ListQuery<ArgoApplicationSetSummary[]>;
	argoProjects: ListQuery<ArgoAppProjectSummary[]>;
	fluxResources: Array<ListQuery<FluxResourceSummary[]>>;
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
	const detectionReady =
		(argoDetection.isSuccess || argoDetection.isError) &&
		(fluxDetection.isSuccess || fluxDetection.isError);
	const data: GitOpsData | undefined = detectionReady
		? {
				argoDetected: argoDetection.data === true,
				apps: argoApps.data ?? [],
				appSets: argoAppSets.data ?? [],
				projects: argoProjects.data ?? [],
				flux: fluxResources.flatMap((query) => query.data ?? []),
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
				argoDetection.isPending ||
				fluxDetection.isPending ||
				(argoDetection.data === true &&
					(argoApps.isPending || argoAppSets.isPending || argoProjects.isPending)) ||
				(fluxDetection.data?.detected === true &&
					fluxResources.some((query) => query.isPending)),
			isError: false,
			error: null,
		},
	};
}
