import type { CSSProperties } from "react";
import type { ResourceKindSelection } from "@/lib/types";
import { discoveredResourceKindKey } from "@/lib/tree-nav";

export const SECTION_LABELS: Record<string, string> = {
	clusterOverview: "Cluster Overview",
	namespaces: "Namespaces",
	workloads: "Workloads",
	network: "Network",
	config: "Config",
	storage: "Storage",
	discovered: "Discovered",
	argo: "Argo CD",
};

export const SIDEBAR_PROVIDER_STYLE = {
	"--sidebar-width": "260px",
} as CSSProperties;

export function resourceKindLabel(kind: ResourceKindSelection): string {
	return typeof kind === "string" ? kind : kind.kind;
}

export function resourceKindLogKey(kind: ResourceKindSelection): string {
	return typeof kind === "string" ? kind : discoveredResourceKindKey(kind);
}

export function hasDiscoveredKind(kinds: ResourceKindSelection[]): boolean {
	return kinds.some((kind) => typeof kind !== "string");
}
