import { onMount } from "svelte";
import { markStartup } from "@/lib/startup-marks";
import { resourceKey } from "@/lib/resource-identity";
import { nodeIdToString, type TreeNodeId } from "@/lib/tree-nav";
import type { ResourceSummary } from "@/lib/types";
import type { ArgoApplicationSummary } from "@/lib/gitops-types";
import type { SavedWorkspace, CreateWorkspaceInput } from "@/lib/workspace-model";
import { getSettingsSnapshot } from "@/lib/settings-store";
import { schedulePathStateWrite, flushPathStateWrite, type PathStateDetailTab, type PathStateResourceDetailState, type PathStateSurfacesState, type PathStateWorkspaceSnapshot } from "@/lib/path-state";
import { confirmStoppingExecSessions, hasOpenExecSessions } from "@/features/live-sessions/openExecSessions.svelte";
import { canLeaveYamlDraft } from "@/features/resource-detail/yamlDraftGuard.svelte";
import { installYamlDraftCloseGuard } from "@/features/resource-detail/yamlDraftCloseGuard";
import { workspaceStore } from "@/features/workspaces/workspaceStore";
import type { HealthFilter } from "@/features/resources/helpers";
import type { IncidentFilter } from "@/features/incidents";
import { onOpenRbacVerifier, type RbacVerifierHandoff } from "@/features/rbac/handoff";
import {
	createWorkspaceNavigation, navigateWorkspace, treeNodeForResource, rememberedWorkspacePath,
	rememberWorkspacePath, workspacePathForWorkspace, workspaceNavigationSnapshot,
	type WorkspaceNavigationIntent, type WorkspaceNavigationState,
} from "./workspaceNavigation";

interface WorkspaceNavigationOptions {
	getWorkspace: () => SavedWorkspace;
	initialPathState: PathStateWorkspaceSnapshot | null;
	onPathStateConsumed: () => void;
	onOpenLauncher: () => void;
	onChangeClusterContext: (workspaceId: string, input: CreateWorkspaceInput) => void;
	currentTitle: () => string;
}

export function workspaceNavigationController({
	getWorkspace, initialPathState, onPathStateConsumed, onOpenLauncher, onChangeClusterContext, currentTitle,
}: WorkspaceNavigationOptions) {
	function initialPathSnapshotForWorkspace() {
		return workspacePathForWorkspace(
			getWorkspace(),
			initialPathState ?? rememberedWorkspacePath(getWorkspace().id),
		);
	}
	const initialPathSnapshot = initialPathSnapshotForWorkspace();
	function initialWorkspaceNavigation() {
		return createWorkspaceNavigation(getWorkspace(), initialPathSnapshot);
	}
	const initialNavigation = initialWorkspaceNavigation();
	const initialDetailPathState = initialPathSnapshot?.detail ?? null;
	const initialSurfacesPathState = initialPathSnapshot?.surfaces ?? null;
	let navigation = $state.raw<WorkspaceNavigationState>(initialNavigation);
	let sidebarNavigationRevision = $state(0);
	let expandedSections = $state<string[]>(
		initialPathSnapshot?.expandedSections ??
			(initialNavigation.selectedNode
				? [nodeIdToString(initialNavigation.selectedNode)]
				: []),
	);
	let navigationOpen = $state(false);
	let resourceDetailPathState = $state<PathStateResourceDetailState | null>(
		initialDetailPathState,
	);
	let surfacesPathState = $state<PathStateSurfacesState | null>(
		initialSurfacesPathState,
	);
	let rbacVerifierHandoff = $state<RbacVerifierHandoff | undefined>();
	let rbacVerifierReturnNavigation = $state<WorkspaceNavigationState | null>(null);
	let rbacVerifierReturnLabel = $state<string | undefined>();
	onMount(() => {
		markStartup("workspace-ready");
		if (initialPathSnapshot) onPathStateConsumed();
		const stopRbacHandoff = onOpenRbacVerifier((handoff) => {
			rbacVerifierReturnNavigation = { ...navigation };
			rbacVerifierReturnLabel = `Return to ${currentTitle()}`;
			rbacVerifierHandoff = handoff;
			applyWorkspaceNavigation({
				type: "selectNode",
				node: { type: "kind", section: "rbac", kind: "Bindings" },
			});
		});
		const closeNarrowNavigation = () => {
			if (window.matchMedia("(min-width: 80rem)").matches) navigationOpen = false;
		};
		window.addEventListener("resize", closeNarrowNavigation);
		window.addEventListener("pagehide", flushPathStateWrite);
		const stopYamlDraftCloseGuard = installYamlDraftCloseGuard();
		window.addEventListener("beforeunload", flushPathStateWrite);
		const flushWhenHidden = () => {
			if (document.visibilityState === "hidden") flushPathStateWrite();
		};
		document.addEventListener("visibilitychange", flushWhenHidden);
		return () => {
			flushPathStateWrite();
			stopRbacHandoff();
			window.removeEventListener("resize", closeNarrowNavigation);
			window.removeEventListener("pagehide", flushPathStateWrite);
			stopYamlDraftCloseGuard();
			window.removeEventListener("beforeunload", flushPathStateWrite);
			document.removeEventListener("visibilitychange", flushWhenHidden);
		};
	});

	function returnFromRbacVerifier() {
		if (rbacVerifierReturnNavigation) navigation = rbacVerifierReturnNavigation;
		rbacVerifierHandoff = undefined;
		rbacVerifierReturnNavigation = null;
		rbacVerifierReturnLabel = undefined;
	}

	function applyWorkspaceNavigation(intent: WorkspaceNavigationIntent, revealSidebar = true) {
		const current = navigation.focusedResource;
		const next = navigateWorkspace(navigation, intent).focusedResource;
		const leavesFocusedResource =
			current !== null && (next === null || resourceKey(next) !== resourceKey(current));
		if (leavesFocusedResource && !canLeaveYamlDraft()) return;
		if (!leavesFocusedResource || !hasOpenExecSessions()) {
			commitWorkspaceNavigation(intent, revealSidebar);
			return;
		}
		void confirmStoppingExecSessions().then((stop) => {
			if (stop) commitWorkspaceNavigation(intent, revealSidebar);
		});
	}

	function commitWorkspaceNavigation(intent: WorkspaceNavigationIntent, revealSidebar: boolean) {
		navigation = navigateWorkspace(navigation, intent);
		// A closed detail's tab must not carry over to the next resource opened without one.
		if (intent.type === "clearResource") resourceDetailPathState = null;
		if (revealSidebar && ["selectNode", "selectResource", "openResources", "openHelmRelease", "openArgo", "openIncidents", "openPortForwards"].includes(intent.type)) {
			sidebarNavigationRevision += 1;
		}
	}

	function openLauncher() {
		if (!canLeaveYamlDraft()) return;
		void confirmStoppingExecSessions().then((stop) => {
			if (!stop) return;
			rememberWorkspacePath(currentPathState());
			onOpenLauncher();
		});
	}

	function changeClusterContext(clusterContext: string) {
		if (clusterContext === getWorkspace().scope.clusterContext || !canLeaveYamlDraft()) return;
		onChangeClusterContext(getWorkspace().id, {
			name: getWorkspace().name,
			clusterContext,
			clusterContexts: [clusterContext],
			namespaces: [],
			kinds: getWorkspace().scope.kinds,
			shortcutPreferences: getWorkspace().scope.shortcutPreferences,
		});
	}

	function currentPathState(): PathStateWorkspaceSnapshot {
		return workspaceNavigationSnapshot(navigation, {
			workspaceId: getWorkspace().id,
			expandedSections,
			detail: resourceDetailPathState,
			surfaces: surfacesPathState,
		});
	}

	$effect(() => {
		const workspacePath = currentPathState();
		rememberWorkspacePath(workspacePath);
		schedulePathStateWrite({
			version: 1,
			runtime: "svelte",
			launcherView: "workspaces",
			workspace: workspacePath,
		});
	});

	function openSettings() {
		applyWorkspaceNavigation({ type: "openSettings" });
	}

	function closeSettings() {
		applyWorkspaceNavigation({ type: "closeSettings" });
	}

	function openResources(
		namespace?: string | string[],
		initialSearch = "",
		initialGitOpsFilter = getWorkspace().scope.gitOpsFilter ?? getWorkspace().scope.argoAppFilter ?? "",
		initialHealthFilter: HealthFilter = "all",
		gitOpsFocusApplication: ArgoApplicationSummary | null = null,
	) {
		if (namespace && !Array.isArray(namespace)) {
			workspaceStore.recordRecentNamespace(
				getWorkspace().id,
				getWorkspace().scope.clusterContext,
				namespace,
			);
		}
		if (gitOpsFocusApplication) {
			workspaceStore.recordRecentApplication(
				getWorkspace().id,
				gitOpsFocusApplication.cluster,
				gitOpsFocusApplication.name,
				gitOpsFocusApplication.namespace,
			);
		}
		applyWorkspaceNavigation({
			type: "openResources",
			namespaces: namespace,
			search: initialSearch,
			gitOpsFilter: initialGitOpsFilter,
			healthFilter: initialHealthFilter,
			gitOpsFocusApplication,
		});
	}

	function openHelmReleaseFromResource(releaseName: string, namespace: string | null, cluster: string) {
		applyWorkspaceNavigation({
			type: "openHelmRelease",
			name: releaseName,
			namespace,
			cluster,
		});
	}

	function clearTargetHelmRelease() {
		applyWorkspaceNavigation({ type: "clearHelmTarget" });
	}

	function clearTargetGitOpsApplication() {
		applyWorkspaceNavigation({ type: "clearGitOpsTarget" });
	}

	function openArgo(argoApp?: string, namespace?: string, cluster = getWorkspace().scope.clusterContext) {
		if (argoApp) {
			workspaceStore.recordRecentApplication(getWorkspace().id, cluster, argoApp, namespace);
		}
		applyWorkspaceNavigation({
			type: "openArgo",
			application: argoApp ? { name: argoApp, namespace, cluster } : undefined,
		});
	}

	function openIncidents(filter: IncidentFilter = "all") {
		applyWorkspaceNavigation({ type: "openIncidents", filter });
	}

	function openPortForwards() {
		applyWorkspaceNavigation({ type: "openPortForwards" });
	}

	function selectNode(nodeId: TreeNodeId, revealSidebar = true) {
		navigationOpen = false;
		if (nodeId.type === "namespace" && nodeId.namespace) {
			workspaceStore.recordRecentNamespace(
				getWorkspace().id,
				getWorkspace().scope.clusterContext,
				nodeId.namespace,
			);
		}
		// Selecting Custom Resources also lists its kinds.
		const key = nodeIdToString(nodeId);
		if (nodeId.type === "section" && nodeId.section === "discovered" && !expandedSections.includes(key)) {
			expandedSections = [...expandedSections, key];
		}
		applyWorkspaceNavigation({ type: "selectNode", node: nodeId }, revealSidebar);
	}

	function selectResource(resource: ResourceSummary, nodeId: TreeNodeId) {
		workspaceStore.recordRecentResource(getWorkspace().id, resource);
		applyWorkspaceNavigation({ type: "selectResource", resource, node: nodeId });
	}

	function detailPathStateForTab(activeTab: PathStateDetailTab): PathStateResourceDetailState {
		const settings = getSettingsSnapshot();
		return {
			activeTab,
			metadataLabelsExpanded: resourceDetailPathState?.metadataLabelsExpanded ?? false,
			metadataAnnotationsExpanded: resourceDetailPathState?.metadataAnnotationsExpanded ?? false,
			selectedContainer: resourceDetailPathState?.selectedContainer ?? "",
			logFilter: resourceDetailPathState?.logFilter ?? "",
			logWrapLines: resourceDetailPathState?.logWrapLines ?? true,
			logLatestFirst: resourceDetailPathState?.logLatestFirst ?? false,
			yamlViewMode: resourceDetailPathState?.yamlViewMode ?? settings.yamlViewModeDefault,
			yamlEncoding: resourceDetailPathState?.yamlEncoding ?? settings.yamlEncodingDefault,
			yamlShowFullDiff: resourceDetailPathState?.yamlShowFullDiff ?? false,
		};
	}

	function inspectResource(resource: ResourceSummary, detailTab?: PathStateDetailTab) {
		resourceDetailPathState = detailTab ? detailPathStateForTab(detailTab) : null;
		workspaceStore.recordRecentResource(getWorkspace().id, resource);
		applyWorkspaceNavigation({ type: "inspectResource", resource });
	}

	function focusResource(
		resource: ResourceSummary,
		source: "explicit" | "restore" = "explicit",
	) {
		if (source === "explicit") {
			workspaceStore.recordRecentResource(getWorkspace().id, resource);
		}
		applyWorkspaceNavigation({ type: "focusResource", resource });
	}

	function togglePinnedResource(resource: ResourceSummary) {
		workspaceStore.togglePinnedResource(getWorkspace().id, resource);
	}

	function openEntryPoint(resource: ResourceSummary) {
		workspaceStore.recordRecentResource(getWorkspace().id, resource);
		applyWorkspaceNavigation({
			type: "selectResource",
			resource,
			node: treeNodeForResource(resource),
		});
	}

	return {
		get navigation() { return navigation; },
		get sidebarNavigationRevision() { return sidebarNavigationRevision; },
		get expandedSections() { return expandedSections; },
		set expandedSections(value: typeof expandedSections) { expandedSections = value; },
		get navigationOpen() { return navigationOpen; },
		set navigationOpen(value: typeof navigationOpen) { navigationOpen = value; },
		get resourceDetailPathState() { return resourceDetailPathState; },
		set resourceDetailPathState(value: typeof resourceDetailPathState) { resourceDetailPathState = value; },
		get surfacesPathState() { return surfacesPathState; },
		set surfacesPathState(value: typeof surfacesPathState) { surfacesPathState = value; },
		get rbacVerifierHandoff() { return rbacVerifierHandoff; },
		set rbacVerifierHandoff(value: typeof rbacVerifierHandoff) { rbacVerifierHandoff = value; },
		get rbacVerifierReturnNavigation() { return rbacVerifierReturnNavigation; },
		get rbacVerifierReturnLabel() { return rbacVerifierReturnLabel; },
		get initialSurfacesPathState() { return initialSurfacesPathState; },
		returnFromRbacVerifier,
		applyWorkspaceNavigation,
		openLauncher,
		changeClusterContext,
		openSettings,
		closeSettings,
		openResources,
		openHelmReleaseFromResource,
		clearTargetHelmRelease,
		clearTargetGitOpsApplication,
		openArgo,
		openIncidents,
		openPortForwards,
		selectNode,
		selectResource,
		inspectResource,
		focusResource,
		togglePinnedResource,
		openEntryPoint,
	};
}
