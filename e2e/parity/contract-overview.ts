import type { JsonObject } from "../../src/lib/types";
import { type StepParsers, boolean, count, isBoolean, isString, object, oneOf, optionalBoolean, string, strings, noPayload } from "./contract-values";

const FILTERS = ["Degraded", "Needs attention", "Healthy", "Unknown", "Not evaluated", "Restart evidence"] as const;

function overviewIdentity(item: JsonObject) {
	return {
		kind: string(item.kind),
		name: string(item.name),
		namespace: string(item.namespace),
		context: item.context === undefined ? "mock-dev" : string(item.context),
		...(item.apiVersion === undefined ? {} : { apiVersion: string(item.apiVersion) }),
	};
}

function overviewActivation(item: JsonObject) {
	return {
		...overviewIdentity(item),
		...(item.key === undefined ? {} : { key: oneOf(["enter", "space"], item.key, "an activation key") }),
	};
}

export const overviewSteps = {
	sidebarToggle: (item) => ({ path: strings(item.path, false) }),
	sidebarSelect: (item) => ({ path: strings(item.path, false) }),
	expectSidebarRow: (item) => ({ path: strings(item.path, false), visible: boolean(item.visible), selected: boolean(item.selected) }),
	collapseSidebar: noPayload,
	expectSidebarBranch: (item) => ({ page: string(item.page), expanded: boolean(item.expanded) }),
	scrollSidebar: (item) => ({ to: oneOf(["top", "bottom"], item.to, "a sidebar scroll target") }),
	expectSidebarReveal: (item) => ({ path: strings(item.path, false), visible: boolean(item.visible), highlighted: boolean(item.highlighted) }),
	expectEmptyLauncher: noPayload,
	startWithoutWorkspace: noPayload,
	expectNoWorkspaceShell: noPayload,
	expectPaletteUnavailable: noPayload,
	expectOverviewShortcuts: (item) => ({ labels: strings(item.labels, true) }),
	overviewShortcut: (item) => ({ label: string(item.label) }),
	expectActivePage: (item) => ({ page: string(item.page) }),
	expectGitOpsSelection: (item) => {
		return {
			name: string(item.name),
			...(item.namespace !== undefined ? { namespace: string(item.namespace) } : {}),
		};
	},
	expectOverviewScope: (item) => ({ labels: strings(item.labels, false) }),
	expectCompareTotal: (item) => ({ title: string(item.title), side: string(item.side), total: count(item.total) }),
	retryOverview: noPayload,
	expectOverviewRetry: noPayload,
	dragKind: (item) => ({ kind: string(item.kind), target: string(item.target) }),
	expectEntryCount: (item) => ({ panel: oneOf(["Pinned", "Recent"], item.panel, "an entry panel"), count: count(item.count) }),
	copyErrorDetail: (item) => ({ error: oneOf(["contexts", "namespaces", "details"], item.error, "an error panel"), expected: string(item.expected) }),
	toggleResourcePin: (item) => {
		return {
			kind: string(item.kind), name: string(item.name), namespace: string(item.namespace),
			...(item.context !== undefined ? { context: string(item.context) } : {}),
			...(item.key !== undefined ? { key: oneOf(["Enter", "Space"] as const, item.key, "a pin key") } : {}),
		};
	},
	expectWorkspaceEntry: (item) => ({ panel: oneOf(["Pinned", "Recent"], item.panel, "an entry panel"), label: string(item.label), index: count(item.index), count: count(item.count) }),
	openWorkspaceEntry: (item) => ({ panel: oneOf(["Pinned", "Recent"], item.panel, "an entry panel"), label: string(item.label), index: count(item.index), count: count(item.count) }),
	reorderKindKey: (item) => ({ kind: string(item.kind), key: oneOf(["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"], item.key, "a reorder key") }),
	resetKindOrder: noPayload,
	configureShellRead: (item) => ({ phase: oneOf(["ready", "contexts-failed", "contexts-empty", "namespaces-failed", "namespaces-credential-failed", "namespaces-empty", "resources-failed", "primary-context-missing", "kind-missing", "payments-api-removed", "payments-deployments-forbidden", "payments-ingress-version-changed", "sources-failed", "overview-owner-chain", "overview-many"], item.phase, "a shell read phase") }),
	openNamespaceList: noPayload,
	openNavigationDrawer: noPayload,
	expectNamespaceRow: (item) => ({ name: string(item.name), age: string(item.age) }),
	resizeViewport: (item) => ({ width: count(item.width), height: count(item.height) }),
	expectNavigationDrawer: (item) => {
		if (!isBoolean(item.open)) throw new Error("Expected drawer state");
		return { open: item.open };
	},
	sidebarKey: (item) => ({ path: strings(item.path, false), key: oneOf(["Enter", "Space", "ArrowLeft", "ArrowRight"], item.key, "a sidebar key") }),
	retryLauncher: (item) => ({ read: oneOf(["contexts", "namespaces"], item.read, "a launcher read") }),
	expectLauncherRetry: (item) => {
		const read = oneOf(["contexts", "namespaces"], item.read, "a launcher read");
		return item.height === undefined ? { read } : { read, height: count(item.height) };
	},
	overviewOperation: (item) => ({ destination: oneOf(["Resources", "Workspaces", "Port Forwards", "Incidents"], item.destination, "an overview operation") }),
	reviewIncidentAction: (item) => ({ index: count(item.index) }),
	failWorkspaceContext: (item) => ({ context: string(item.context) }),
	expectWorkspaceReadWarning: (item) => ({ context: string(item.context) }),
	holdWorkspaceContext: (item) => ({ context: string(item.context) }),
	releaseWorkspaceContext: (item) => ({ context: string(item.context) }),
	expectContextsLoading: (item) => ({ contexts: strings(item.contexts, true) }),
	editSavedWorkspace: (item) => ({ name: string(item.name) }),
	expectNoSavedWorkspace: (item) => ({ name: string(item.name) }),
	saveWorkspace: noPayload,
	cancelWorkspaceEdit: noPayload,
	toggleWorkspaceContext: (item) => ({ context: string(item.context) }),
	chooseClusterContext: (item) => ({ context: string(item.context) }),
	expectDetailContext: (item) => ({ context: string(item.context) }),
	expectOverviewHealth: (item) => ({ total: count(item.total) }),
	refreshPage: (item) => ({ page: oneOf(["RBAC", "Incidents"], item.page, "a refresh button page") }),
	replayPageReads: (item) => ({ phase: oneOf(["base", "updated", "failed"], item.phase, "a page read phase") }),
	focusStalePage: (item) => ({ page: oneOf(["GitOps", "Helm", "RBAC", "Incidents"], item.page, "a refreshing page") }),
	expectPageText: (item) => ({ text: string(item.text) }),
	expectWorkspaceExport: (item) => ({ name: string(item.name) }),
	exportSavedWorkspace: (item) => ({ name: string(item.name) }),
	expectExportedShortcut: (item) => ({ name: string(item.name), shortcut: object(item.shortcut) }),
	searchCustomKinds: (item) => ({ value: item.value === "" ? "" : string(item.value) }),
	searchKindPicker: (item) => ({ value: item.value === "" ? "" : string(item.value) }),
	expectCustomGroup: (item) => ({ group: string(item.group), visible: boolean(item.visible) }),
	toggleKind: (item) => ({ kind: string(item.kind), checked: boolean(item.checked) }),
	expectKindChecked: (item) => ({ kind: string(item.kind), checked: boolean(item.checked) }),
	openKindPicker: noPayload,
	closeKindPicker: noPayload,
	selectAllKinds: noPayload,
	openPage: (item) => ({ page: string(item.page) }),
	expectDetailTab: (item) => ({ tab: string(item.tab) }),
	expectPaletteResource: (item) => {
		if (!isString(item.namespace)) throw new Error("Expected a resource namespace, empty for cluster resources");
		return { kind: string(item.kind), name: string(item.name), namespace: item.namespace };
	},
	expectNoPaletteResource: (item) => {
		if (!isString(item.namespace)) throw new Error("Expected a resource namespace, empty for cluster resources");
		return { kind: string(item.kind), name: string(item.name), namespace: item.namespace };
	},
	choosePaletteResource: (item) => {
		if (!isString(item.namespace)) throw new Error("Expected a resource namespace, empty for cluster resources");
		return { kind: string(item.kind), name: string(item.name), namespace: item.namespace };
	},
	openLauncher: noPayload,
	createWorkspace: noPayload,
	nameWorkspace: (item) => ({ value: string(item.value) }),
	searchPalette: (item) => ({ value: string(item.value) }),
	openSavedWorkspace: (item) => ({ name: string(item.name) }),
	expectSavedWorkspace: (item) => ({ name: string(item.name), summary: string(item.summary) }),
	expectContextUnavailable: (item) => ({ name: string(item.name) }),
	selectWorkspaceContext: (item) => ({ context: string(item.context) }),
	toggleWorkspaceNamespace: (item) => ({ namespace: string(item.namespace) }),
	expectOpenWorkspace: (item) => ({ name: string(item.name), context: string(item.context) }),
	openPalette: (item) => ({ via: oneOf(["button", "shortcut"], item.via, "how the palette opens") }),
	expectPaletteEntry: (item) => ({ label: string(item.label) }),
	expectNoPaletteEntry: (item) => ({ label: string(item.label) }),
	choosePaletteEntry: (item) => ({ label: string(item.label) }),
	expandNav: (item) => ({ page: string(item.page) }),
	expectRemoteHost: (item) => ({ url: string(item.url), disconnected: optionalBoolean(item.disconnected) }),
	expectRemoteHostTooltip: (item) => ({ value: string(item.value) }),
	expectNoRemoteHost: noPayload,
	dropRemoteConnection: noPayload,
	overviewHealth: (item) => ({ filter: oneOf(FILTERS, item.filter, "a health filter") }),
	overviewNamespace: (item) => ({ namespace: string(item.namespace) }),
	expectOverviewProblem: (item) => ({ ...overviewIdentity(item), ready: string(item.ready), status: string(item.status) }),
	openOverviewProblem: overviewActivation,
	toggleOverviewProblem: overviewActivation,
	expectNoOverviewProblem: overviewIdentity,
} satisfies StepParsers;
