import type { ArgoResourceFilter } from "@/features/gitops/argo-workspace-model";
import type { ArgoApplicationSummary } from "@/lib/gitops-types";
import { isCustomResourceKind, kindKey, sameKinds } from "@/lib/kind-identity";
import type { PathStateResourceBrowserState } from "@/lib/path-state";
import { getSettingsSnapshot } from "@/lib/settings-store";
import type { ResourceKindSelection } from "@/lib/types";
import type { HealthFilter } from "./helpers";
import { initialOwnershipMapOpen, newlyAvailableKinds } from "./resourceBrowserModel";

interface ScopeOptions {
	initialNamespaces: string[];
	initialKinds: ResourceKindSelection[];
	availableKinds: ResourceKindSelection[];
	initialSearch: string;
	initialGitOpsFilter: string;
	initialHealthFilter: HealthFilter;
	gitOpsFocusApplication: ArgoApplicationSummary | null;
	kindScopeLocked: boolean;
	initialPathState: PathStateResourceBrowserState | null;
	selectAddedKinds: boolean;
	customResourcesEnabled: boolean;
}

interface BrowserScope extends Omit<PathStateResourceBrowserState, "collapsedGroups"> {
	collapsedGroups: Set<string>;
	filterSearch: string;
	argoResourceFilter: ArgoResourceFilter;
	contextFilter: string[];
}

export function resourceBrowserScope(
	options: ScopeOptions,
	scopeReadReady: () => boolean,
	savePathState: (state: PathStateResourceBrowserState) => void,
	safePageIndex: () => number,
) {
	const scope = $state<BrowserScope>({
		selectedNamespaces: [],
		selectedKinds: [],
		search: "",
		filterSearch: "",
		gitOpsFilter: "",
		healthFilter: "all",
		argoResourceFilter: "none",
		sortColumn: "name",
		sortDesc: false,
		pageIndex: 0,
		scopeEditorOpen: false,
		collapsedGroups: new Set<string>(),
		selectedTopologyNodeId: null,
		topologyMode: "ownership",
		mapPanelOpen: initialOwnershipMapOpen(options.initialPathState, getSettingsSnapshot().showOwnershipMapByDefault),
		tablePanelOpen: true,
		contextFilter: [],
	});
	let appliedScopeKey = $state("");
	let appliedAvailableKinds = $state.raw<ResourceKindSelection[]>([]);
	let initialPathStateConsumed = false;

	const incomingScopeKey = $derived(
		JSON.stringify({
			namespaces: [...options.initialNamespaces].sort(),
			kinds: options.initialKinds.map(kindKey).sort(),
			search: options.initialSearch,
			gitOpsFilter: options.initialGitOpsFilter,
			healthFilter: options.initialHealthFilter,
			argoApplication: options.gitOpsFocusApplication
				? `${options.gitOpsFocusApplication.namespace ?? ""}:${options.gitOpsFocusApplication.name}`
				: null,
		}),
	);
	$effect(() => {
		if (appliedScopeKey === incomingScopeKey) return;
		const pathState = initialPathStateConsumed || options.kindScopeLocked ? null : options.initialPathState;
		scope.selectedNamespaces = pathState ? [...pathState.selectedNamespaces] : [...options.initialNamespaces];
		scope.selectedKinds = pathState ? [...pathState.selectedKinds] : [...options.initialKinds];
		appliedScopeKey = incomingScopeKey;
		appliedAvailableKinds = options.selectAddedKinds ? options.initialKinds : options.availableKinds;
		scope.search = pathState?.search ?? options.initialSearch;
		scope.filterSearch = scope.search;
		scope.gitOpsFilter = pathState?.gitOpsFilter ?? options.initialGitOpsFilter;
		scope.healthFilter = pathState?.healthFilter ?? options.initialHealthFilter;
		scope.argoResourceFilter = "none";
		scope.sortColumn = pathState?.sortColumn ?? "name";
		scope.sortDesc = pathState?.sortDesc ?? false;
		scope.pageIndex = pathState?.pageIndex ?? 0;
		scope.scopeEditorOpen = pathState?.scopeEditorOpen ?? false;
		scope.collapsedGroups = new Set(pathState?.collapsedGroups ?? []);
		scope.selectedTopologyNodeId = pathState?.selectedTopologyNodeId ?? null;
		scope.topologyMode = pathState?.topologyMode ?? "ownership";
		scope.mapPanelOpen = initialOwnershipMapOpen(pathState, getSettingsSnapshot().showOwnershipMapByDefault);
		scope.tablePanelOpen = pathState?.tablePanelOpen ?? true;
		initialPathStateConsumed = true;
	});
	/** Selects kinds that became available since they were last applied. */
	function selectNewlyAvailableKinds() {
		if (sameKinds(appliedAvailableKinds, options.availableKinds)) return;
		const additions = options.selectAddedKinds
			? newlyAvailableKinds(options.availableKinds, appliedAvailableKinds, scope.selectedKinds)
			: [];
		appliedAvailableKinds = options.availableKinds;
		if (additions.length > 0) scope.selectedKinds = [...scope.selectedKinds, ...additions];
	}
	$effect(() => {
		if (appliedScopeKey !== "") selectNewlyAvailableKinds();
	});

	$effect(() => {
		if (options.kindScopeLocked) {
			if (scope.selectedKinds.length !== options.initialKinds.length ||
				scope.selectedKinds.some((kind, index) => kindKey(kind) !== kindKey(options.initialKinds[index]))) {
				scope.selectedKinds = [...options.initialKinds];
			}
			appliedAvailableKinds = options.availableKinds;
			return;
		}
		if (!options.customResourcesEnabled) {
			const builtinKinds = scope.selectedKinds.filter((kind) => !isCustomResourceKind(kind));
			if (builtinKinds.length !== scope.selectedKinds.length) scope.selectedKinds = builtinKinds;
			appliedAvailableKinds = options.availableKinds;
			return;
		}
		if (appliedScopeKey !== "" && scopeReadReady()) {
			selectNewlyAvailableKinds();
		}
	});
	$effect(() => {
		const value = scope.search;
		const frame = requestAnimationFrame(() => {
			if (scope.filterSearch === value) return;
			scope.filterSearch = value;
			scope.pageIndex = 0;
		});
		return () => cancelAnimationFrame(frame);
	});
	$effect(() => {
		savePathState({
			selectedNamespaces: scope.selectedNamespaces,
			selectedKinds: scope.selectedKinds,
			search: scope.search,
			gitOpsFilter: scope.gitOpsFilter,
			healthFilter: scope.healthFilter,
			sortColumn: scope.sortColumn,
			sortDesc: scope.sortDesc,
			pageIndex: safePageIndex(),
			scopeEditorOpen: scope.scopeEditorOpen,
			collapsedGroups: [...scope.collapsedGroups],
			topologyMode: scope.topologyMode,
			selectedTopologyNodeId: scope.selectedTopologyNodeId,
			mapPanelOpen: scope.mapPanelOpen,
			tablePanelOpen: scope.tablePanelOpen,
		});
	});

	return scope;
}
