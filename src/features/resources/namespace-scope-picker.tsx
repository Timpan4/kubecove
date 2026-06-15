import { useQuery } from "@tanstack/react-query";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useSettingsState } from "@/lib/settings";
import { createTauriClient, listNamespaces } from "@/lib/tauri";
import { queryKeys } from "@/lib/queryKeys";
import {
	PickerHeader,
	PickerStatus,
	ScopeOption,
} from "./scope-filter-shared";

export function NamespaceScopePicker({
	clusterContext,
	selectedNamespaces,
	onNamespaceChange,
}: {
	clusterContext: string;
	selectedNamespaces: string[];
	onNamespaceChange: (namespaces: string[]) => void;
}) {
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	const {
		data: namespaces = [],
		isPending,
		isError,
		error,
		refetch,
	} = useQuery({
		queryKey: queryKeys.namespaces(clusterContext, kubeconfigEnvVar),
		queryFn: () =>
			listNamespaces(createTauriClient(), clusterContext, kubeconfigEnvVar),
		enabled: Boolean(clusterContext),
	});
	const loading = Boolean(clusterContext) && isPending;
	const errorMessage = isError
		? error instanceof Error
			? error.message
			: "Failed to load namespaces"
		: null;

	const allSelected =
		namespaces.length > 0 && selectedNamespaces.length === namespaces.length;

	return (
		<div className="flex min-h-0 flex-col">
			<PickerHeader
				title="Namespaces"
				allSelected={allSelected}
				onToggleAll={() =>
					onNamespaceChange(
						allSelected ? [] : namespaces.map((namespace) => namespace.name),
					)
				}
			/>
			<PickerStatus
				loading={loading}
				loadingLabel="Loading namespaces..."
				error={errorMessage}
				onRetry={() => void refetch()}
			/>
			<ScrollArea className="h-64 pr-2">
				<ul className="m-0 flex list-none flex-col gap-1 p-0">
					{namespaces.map((namespace) => {
						const checked = selectedNamespaces.includes(namespace.name);
						return (
							<ScopeOption
								key={namespace.name}
								id={`scope-namespace-${namespace.name}`}
								label={namespace.name}
								checked={checked}
								onToggle={() =>
									onNamespaceChange(
										checked
											? selectedNamespaces.filter((name) => name !== namespace.name)
											: [...selectedNamespaces, namespace.name],
									)
								}
							/>
						);
					})}
				</ul>
			</ScrollArea>
		</div>
	);
}
