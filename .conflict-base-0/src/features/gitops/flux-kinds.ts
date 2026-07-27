export {
	FLUX_KIND_LABELS,
	fluxKindFromLabel,
	isFluxKindLabel,
} from "./gitops-nav";

export function fluxStatusTone(status?: string | null) {
	if (status === "True") return "success";
	if (status === "False") return "error";
	return "warning";
}
