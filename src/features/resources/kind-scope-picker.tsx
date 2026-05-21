import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { createTauriClient, listResourceKinds } from "@/lib/tauri";
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
	const [discoveredKinds, setDiscoveredKinds] = useState<DiscoveredResourceKind[]>([]);
	const [loading, setLoading] = useState(true);
	const [error, setError] = useState<string | null>(null);
	const requestSeqRef = useRef(0);

	const loadKinds = useCallback(async () => {
		const requestSeq = ++requestSeqRef.current;
		if (!clusterContext) {
			setDiscoveredKinds([]);
			setError(null);
			setLoading(false);
			return;
		}

		setDiscoveredKinds([]);
		setLoading(true);
		setError(null);
		try {
			const kinds = await listResourceKinds(createTauriClient(), clusterContext);
			if (requestSeq !== requestSeqRef.current) return;
			setDiscoveredKinds(
				kinds
					.filter((kind) => !CURATED_DISCOVERY_KEYS.has(kindDiscoveryKey(kind)))
					.sort((a, b) =>
						a.kind.localeCompare(b.kind) ||
						a.apiVersion.localeCompare(b.apiVersion) ||
						a.plural.localeCompare(b.plural),
					),
			);
		} catch (err) {
			if (requestSeq !== requestSeqRef.current) return;
			setError(err instanceof Error ? err.message : "Failed to load resource kinds");
		} finally {
			if (requestSeq === requestSeqRef.current) setLoading(false);
		}
	}, [clusterContext]);

	useEffect(() => {
		void loadKinds();
	}, [loadKinds]);

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

	const toggleKind = (kind: ResourceKindSelection) => {
		const kindKey = resourceKindFetchKey(kind);
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
				onToggleAll={() => onKindChange(allSelected ? [] : availableKinds)}
			/>
			<PickerStatus
				loading={loading}
				loadingLabel="Loading discovered kinds..."
				error={error}
				onRetry={loadKinds}
			/>
			<ScrollArea className="h-64 pr-2">
				<ul className="m-0 flex list-none flex-col gap-1 p-0">
					{builtInKinds.map((kind) => (
						<ScopeOption
							key={resourceKindFetchKey(kind)}
							id={`scope-kind-${resourceKindFetchKey(kind)}`}
							label={resourceKindLabel(kind)}
							checked={selectedKindKeys.has(resourceKindFetchKey(kind))}
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
