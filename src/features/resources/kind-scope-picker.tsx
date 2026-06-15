import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { useSettingsState } from "@/lib/settings";
import { createTauriClient, listResourceKinds } from "@/lib/tauri";
import { queryKeys } from "@/lib/queryKeys";
import {
	CLUSTER_SCOPED_KINDS,
	SUPPORTED_KINDS,
	type DiscoveredResourceKind,
	type ResourceKindSelection,
} from "@/lib/types";
import {
	resourceKindFetchKey,
	resourceKindLabel,
} from "./helpers";
import {
	PickerHeader,
	PickerStatus,
	ScopeOption,
} from "./scope-filter-shared";

const CURATED_DISCOVERY_KEYS = new Set([
	"/Pod",
	"/Service",
	"/ConfigMap",
	"/Secret",
	"/PersistentVolumeClaim",
	"/Node",
	"/PersistentVolume",
	"apps/Deployment",
	"apps/StatefulSet",
	"apps/DaemonSet",
	"batch/Job",
	"batch/CronJob",
	"networking.k8s.io/Ingress",
	"storage.k8s.io/StorageClass",
	"argoproj.io/Application",
	"argoproj.io/ApplicationSet",
	"argoproj.io/AppProject",
]);

function kindDiscoveryKey(kind: DiscoveredResourceKind): string {
	return `${kind.group}/${kind.kind}`;
}

export function KindScopePicker({
	clusterContext,
	selectedKinds,
	onKindChange,
}: {
	clusterContext: string;
	selectedKinds: ResourceKindSelection[];
	onKindChange: (kinds: ResourceKindSelection[]) => void;
}) {
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	const {
		data: fetchedKinds = [],
		isPending,
		isError,
		error,
		refetch,
	} = useQuery({
		queryKey: queryKeys.resourceKinds(clusterContext, kubeconfigEnvVar),
		queryFn: () =>
			listResourceKinds(createTauriClient(), clusterContext, kubeconfigEnvVar),
		enabled: Boolean(clusterContext),
	});
	const discoveredKinds = useMemo(
		() =>
			fetchedKinds
				.filter((kind) => !CURATED_DISCOVERY_KEYS.has(kindDiscoveryKey(kind)))
				.toSorted((a, b) =>
					a.kind.localeCompare(b.kind) ||
					a.apiVersion.localeCompare(b.apiVersion) ||
					a.plural.localeCompare(b.plural),
				),
		[fetchedKinds],
	);
	const loading = Boolean(clusterContext) && isPending;
	const errorMessage = isError
		? error instanceof Error
			? error.message
			: "Failed to load resource kinds"
		: null;

	const builtInKinds = useMemo<ResourceKindSelection[]>(
		() => [...SUPPORTED_KINDS, ...CLUSTER_SCOPED_KINDS],
		[],
	);
	const availableKinds = useMemo<ResourceKindSelection[]>(
		() => [...builtInKinds, ...discoveredKinds],
		[builtInKinds, discoveredKinds],
	);
	const selectedKindKeys = useMemo(
		() => new Set(selectedKinds.map(resourceKindFetchKey)),
		[selectedKinds],
	);
	const allSelected =
		availableKinds.length > 0 &&
		availableKinds.every((kind) => selectedKindKeys.has(resourceKindFetchKey(kind)));
	const onlyOneKindSelected = selectedKinds.length === 1;

	const toggleKind = (kind: ResourceKindSelection) => {
		const kindKey = resourceKindFetchKey(kind);
		if (selectedKindKeys.has(kindKey) && onlyOneKindSelected) return;
		onKindChange(
			selectedKindKeys.has(kindKey)
				? selectedKinds.filter((selectedKind) => resourceKindFetchKey(selectedKind) !== kindKey)
				: [...selectedKinds, kind],
		);
	};

	return (
		<div className="flex min-h-0 flex-col">
			<PickerHeader
				title="Resource kinds"
				allSelected={allSelected}
				toggleAllDisabled={allSelected}
				disabledLabel="All Selected"
				onToggleAll={() => onKindChange(allSelected ? [] : availableKinds)}
			/>
			<PickerStatus
				loading={loading}
				loadingLabel="Loading discovered kinds..."
				error={errorMessage}
				onRetry={() => void refetch()}
			/>
			<ScrollArea className="h-64 pr-2">
				<ul className="m-0 flex list-none flex-col gap-1 p-0">
					{builtInKinds.map((kind) => (
						<ScopeOption
							key={resourceKindFetchKey(kind)}
							id={`scope-kind-${resourceKindFetchKey(kind)}`}
							label={resourceKindLabel(kind)}
							checked={selectedKindKeys.has(resourceKindFetchKey(kind))}
							disabled={selectedKindKeys.has(resourceKindFetchKey(kind)) && onlyOneKindSelected}
							onToggle={() => toggleKind(kind)}
						/>
					))}
					{discoveredKinds.length > 0 && (
						<>
							<li className="py-2">
								<Separator />
							</li>
							{discoveredKinds.map((kind) => (
								<ScopeOption
									key={resourceKindFetchKey(kind)}
									id={`scope-kind-${resourceKindFetchKey(kind)}`}
									label={resourceKindLabel(kind)}
									checked={selectedKindKeys.has(resourceKindFetchKey(kind))}
									disabled={selectedKindKeys.has(resourceKindFetchKey(kind)) && onlyOneKindSelected}
									onToggle={() => toggleKind(kind)}
								/>
							))}
						</>
					)}
				</ul>
			</ScrollArea>
		</div>
	);
}
