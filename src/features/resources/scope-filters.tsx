import {
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
	type ReactNode,
} from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Field, FieldLabel } from "@/components/ui/field";
import {
	Popover,
	PopoverContent,
	PopoverTrigger,
} from "@/components/ui/popover";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { createTauriClient, listNamespaces, listResourceKinds } from "@/lib/tauri";
import {
	CLUSTER_SCOPED_KINDS,
	SUPPORTED_KINDS,
	type DiscoveredResourceKind,
	type NamespaceSummary,
	type ResourceKindSelection,
} from "@/lib/types";
import { cn } from "@/lib/utils";
import {
	resourceKindFetchKey,
	resourceKindLabel,
	type ScopePill,
} from "./helpers";

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

function EditableScopePill({
	pill,
	children,
}: {
	pill: ScopePill;
	children: ReactNode;
}) {
	return (
		<Popover>
			<PopoverTrigger asChild>
				<button
					type="button"
					className="flex h-8 max-w-full items-center gap-1.5 rounded-sm border border-slate-700/80 bg-slate-950/45 px-2.5 text-xs shadow-none transition-colors hover:border-ring hover:bg-muted/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 data-[state=open]:border-primary/60"
					aria-label={`Edit ${pill.label.toLowerCase()} filter`}
				>
					<span className="text-muted-foreground">{pill.label}</span>
					<strong className="min-w-0 truncate font-semibold text-foreground">
						{pill.value}
					</strong>
				</button>
			</PopoverTrigger>
			<PopoverContent align="start" className="w-80 p-3">
				{children}
			</PopoverContent>
		</Popover>
	);
}

function PickerHeader({
	title,
	allSelected,
	onToggleAll,
}: {
	title: string;
	allSelected: boolean;
	onToggleAll: () => void;
}) {
	return (
		<div className="mb-3 flex items-center justify-between">
			<h3 className="m-0 text-xs font-semibold uppercase text-muted-foreground">
				{title}
			</h3>
			<Button
				type="button"
				variant="outline"
				size="sm"
				className="h-7 px-2 text-[0.625rem]"
				onClick={onToggleAll}
			>
				{allSelected ? "Deselect All" : "Select All"}
			</Button>
		</div>
	);
}

function PickerStatus({
	loading,
	loadingLabel,
	error,
	onRetry,
}: {
	loading: boolean;
	loadingLabel: string;
	error: string | null;
	onRetry: () => void;
}) {
	if (loading) {
		return (
			<div className="mb-3 flex items-center gap-2 text-sm text-muted-foreground">
				<Spinner className="size-4" />
				{loadingLabel}
			</div>
		);
	}
	if (!error) return null;
	return (
		<div className="mb-3 flex flex-col gap-2 text-sm text-destructive">
			<span>{error}</span>
			<Button
				type="button"
				variant="outline"
				size="sm"
				className="w-fit"
				onClick={onRetry}
			>
				Retry
			</Button>
		</div>
	);
}

function ScopeOption({
	id,
	label,
	checked,
	onToggle,
}: {
	id: string;
	label: string;
	checked: boolean;
	onToggle: () => void;
}) {
	return (
		<li
			className={cn(
				"rounded-md p-2 text-sm transition-colors hover:bg-accent",
				checked && "bg-accent",
			)}
		>
			<Field orientation="horizontal" className="items-center gap-2">
				<Checkbox id={id} checked={checked} onCheckedChange={onToggle} />
				<FieldLabel
					htmlFor={id}
					className="min-w-0 flex-1 cursor-pointer truncate font-normal"
				>
					{label}
				</FieldLabel>
			</Field>
		</li>
	);
}

function NamespaceScopePicker({
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
	const requestSeqRef = useRef(0);

	const loadNamespaces = useCallback(async () => {
		const requestSeq = ++requestSeqRef.current;
		if (!clusterContext) {
			setNamespaces([]);
			setLoading(false);
			return;
		}

		setLoading(true);
		setError(null);
		try {
			const ns = await listNamespaces(createTauriClient(), clusterContext);
			if (requestSeq !== requestSeqRef.current) return;
			setNamespaces(ns);
		} catch (err) {
			if (requestSeq !== requestSeqRef.current) return;
			setError(err instanceof Error ? err.message : "Failed to load namespaces");
		} finally {
			if (requestSeq === requestSeqRef.current) setLoading(false);
		}
	}, [clusterContext]);

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

function kindDiscoveryKey(kind: DiscoveredResourceKind): string {
	return `${kind.group}/${kind.kind}`;
}

function KindScopePicker({
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
			setLoading(false);
			return;
		}

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
