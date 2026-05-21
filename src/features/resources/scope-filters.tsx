import { Badge } from "@/components/ui/badge";
import type { ResourceKindSelection } from "@/lib/types";
import type { ScopePill } from "./helpers";
import { KindScopePicker } from "./kind-scope-picker";
import { NamespaceScopePicker } from "./namespace-scope-picker";
import { EditableScopePill } from "./scope-filter-shared";

export function ResourceScopePills({
	pills,
	clusterContext,
	selectedNamespaces,
	selectedKinds,
	onNamespaceChange,
	onKindChange,
}: {
	pills: ScopePill[];
	clusterContext: string;
	selectedNamespaces: string[];
	selectedKinds: ResourceKindSelection[];
	onNamespaceChange: (namespaces: string[]) => void;
	onKindChange: (kinds: ResourceKindSelection[]) => void;
}) {
	return (
		<div
			className="flex min-h-8 flex-wrap items-center gap-2"
			aria-label="Current resource scope"
		>
			{pills.map((pill) => {
				if (pill.kind === "namespaces") {
					return (
						<EditableScopePill key={pill.kind} pill={pill}>
							<NamespaceScopePicker
								clusterContext={clusterContext}
								selectedNamespaces={selectedNamespaces}
								onNamespaceChange={onNamespaceChange}
							/>
						</EditableScopePill>
					);
				}
				if (pill.kind === "kinds") {
					return (
						<EditableScopePill key={pill.kind} pill={pill}>
							<KindScopePicker
								clusterContext={clusterContext}
								selectedKinds={selectedKinds}
								onKindChange={onKindChange}
							/>
						</EditableScopePill>
					);
				}
				return (
					<Badge
						key={pill.kind}
						variant="outline"
						className="h-8 max-w-full gap-1.5 rounded-sm border-slate-700/80 bg-slate-950/45 px-2.5 text-xs shadow-none"
					>
						<span className="text-muted-foreground">{pill.label}</span>
						<strong className="min-w-0 truncate font-semibold text-foreground">
							{pill.value}
						</strong>
					</Badge>
				);
			})}
		</div>
	);
}
