import { ArgoCDPanel } from "@/features/argo/ArgoCDPanel";
import type {
	ArgoAppProjectSummary,
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	FluxResourceSummary,
} from "@/lib/types";
import { isFluxKindLabel } from "./flux-kinds";
import { FluxPanel } from "./FluxPanel";

type ArgoSummaryItem =
	| ArgoApplicationSummary
	| ArgoApplicationSetSummary
	| ArgoAppProjectSummary;

export function GitOpsPanel({
	clusterContext,
	selectedGitOpsItem,
	onGitOpsItemSelect,
	selectedGitOpsKind,
	selectedFluxResource,
	onFluxResourceSelect,
}: {
	clusterContext: string;
	selectedGitOpsItem: ArgoSummaryItem | null;
	onGitOpsItemSelect: (item: ArgoSummaryItem) => void;
	selectedGitOpsKind: string | null;
	selectedFluxResource: FluxResourceSummary | null;
	onFluxResourceSelect: (resource: FluxResourceSummary) => void;
}) {
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
