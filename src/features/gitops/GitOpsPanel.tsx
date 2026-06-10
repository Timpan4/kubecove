import { ArgoCDPanel } from "@/features/argo/ArgoCDPanel";
import type {
	ArgoAppProjectSummary,
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	FluxResourceSummary,
} from "@/lib/types";
import { isFluxKindLabel } from "./flux-kinds";
import { FluxPanel } from "./FluxPanel";
import { GitOpsOverview } from "./GitOpsOverview";

type ArgoSummaryItem =
	| ArgoApplicationSummary
	| ArgoApplicationSetSummary
	| ArgoAppProjectSummary;

export function GitOpsPanel({
	clusterContext,
	selectedGitOpsItem,
	onGitOpsItemSelect,
	onOpenArgoApplicationResources,
	selectedGitOpsKind,
	selectedFluxResource,
	onFluxResourceSelect,
}: {
	clusterContext: string;
	selectedGitOpsItem: ArgoSummaryItem | null;
	onGitOpsItemSelect: (item: ArgoSummaryItem) => void;
	onOpenArgoApplicationResources: (app: ArgoApplicationSummary) => void;
	selectedGitOpsKind: string | null;
	selectedFluxResource: FluxResourceSummary | null;
	onFluxResourceSelect: (resource: FluxResourceSummary) => void;
}) {
	if (!selectedGitOpsKind) {
		return (
			<GitOpsOverview
				clusterContext={clusterContext}
				selectedGitOpsItem={selectedGitOpsItem}
				selectedFluxResource={selectedFluxResource}
				onGitOpsItemSelect={onGitOpsItemSelect}
				onOpenArgoApplicationResources={onOpenArgoApplicationResources}
				onFluxResourceSelect={onFluxResourceSelect}
			/>
		);
	}

	if (isFluxKindLabel(selectedGitOpsKind)) {
		return (
			<FluxPanel
				clusterContext={clusterContext}
				selectedKindLabel={selectedGitOpsKind}
				selectedFluxResource={selectedFluxResource}
				onFluxResourceSelect={onFluxResourceSelect}
			/>
		);
	}

	return (
		<ArgoCDPanel
			clusterContext={clusterContext}
			selectedArgoItem={selectedGitOpsItem}
			onArgoItemSelect={onGitOpsItemSelect}
			selectedArgoKind={selectedGitOpsKind}
		/>
	);
}
