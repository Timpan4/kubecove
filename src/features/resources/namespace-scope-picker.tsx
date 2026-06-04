import { useCallback, useEffect, useRef, useState } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useSettingsState } from "@/lib/settings";
import { createTauriClient, listNamespaces } from "@/lib/tauri";
import type { NamespaceSummary } from "@/lib/types";
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
	const [namespaces, setNamespaces] = useState<NamespaceSummary[]>([]);
	const [loading, setLoading] = useState(true);
	const [error, setError] = useState<string | null>(null);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigEnvVar);
	const requestSeqRef = useRef(0);

	const loadNamespaces = useCallback(async () => {
		if (!clusterContext) {
			requestSeqRef.current += 1;
			setNamespaces([]);
			setError(null);
			setLoading(false);
			return;
		}

		const requestSeq = ++requestSeqRef.current;
		setNamespaces([]);
		setLoading(true);
		setError(null);
		try {
			const ns = await listNamespaces(
				createTauriClient(),
				clusterContext,
				kubeconfigEnvVar,
			);
			if (requestSeq === requestSeqRef.current) setNamespaces(ns);
		} catch (err) {
			if (requestSeq === requestSeqRef.current) {
				setError(err instanceof Error ? err.message : "Failed to load namespaces");
			}
		} finally {
			if (requestSeq === requestSeqRef.current) setLoading(false);
		}
	}, [clusterContext, kubeconfigEnvVar]);

	useEffect(() => {
		void loadNamespaces();
	}, [loadNamespaces]);

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
				error={error}
				onRetry={loadNamespaces}
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
