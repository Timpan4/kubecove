import type { JsonObject } from "../../src/lib/types";
import { type StepParsers, boolean, count, isBoolean, isNumber, isString, oneOf, optionalBoolean, string, strings, noPayload, withOptional } from "./contract-values";

const HEALTH_FILTER_LABELS = ["Total", "Healthy", "Unknown", "Not evaluated", "Needs attention", "Degraded", "Restart evidence"] as const;
const TABLE_COLUMNS = ["Name", "Status", "Context", "Namespace", "Kind", "Ready", "Restarts", "CPU", "Memory", "Owner", "Age"] as const;
const SORT_COLUMNS = ["Name", "Status", "Context", "Namespace", "Kind", "Ready", "Restarts", "CPU", "Memory", "Age"] as const;
const PANEL_CONTROLS = [
    "Collapse ownership map", "Show ownership map", "Collapse resource table", "Show resource table",
] as const;
const MAP_KEYS = ["Tab", "Shift+Tab", "Enter", "Space"] as const;
const MAP_CONTEXT_KEYS = ["Enter", "Space", "ArrowDown", "ArrowUp", "Home", "End", "Escape", "Tab", "Shift+Tab"] as const;
const MAP_CONTROLS = ["Zoom In", "Zoom Out", "Fit View"] as const;
const DETAIL_RESIZE_KEYS = ["ArrowLeft", "ArrowRight", "Home", "End"] as const;

function resourceIdentity(item: JsonObject) {
	return { kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) };
}

function clusterResourceIdentity(item: JsonObject) {
	return { kind: string(item.kind), name: string(item.name) };
}

export const resourcesSteps = {
	expectForegroundLoading: (item) => {
		const sliding = optionalBoolean(item.sliding);
		return withOptional({
			active: boolean(item.active),
		}, {
			sliding,
		});
	},
	expectMotion: (item) => ({ moving: boolean(item.moving) }),
	setReducedMotion: (item) => ({ reduce: boolean(item.reduce) }),
	expectFriendlyError: (item) => {
		return withOptional({
			title: string(item.title),
		}, {
			next: item.next !== undefined ? string(item.next) : undefined,
			summary: item.summary !== undefined ? string(item.summary) : undefined,
		});
	},
	inspectRequiredPermission: noPayload,
	returnToSource: noPayload,
	dismissCopyToast: noPayload,
	expectPermissionTarget: (item) => ({ verb: string(item.verb), resource: string(item.resource) }),
	copyResourceName: resourceIdentity,
	expectCopyToast: (item) => ({ text: string(item.text) }),
	expectTopologyDenied: noPayload,
	expectWatchStart: (item) => ({ kind: string(item.kind), resourceVersion: item.resourceVersion === null ? null : string(item.resourceVersion) }),
	runTopologySpike: noPayload,
	expectTopologySpikeText: (item) => ({ value: string(item.value) }),
	expectResourceStatus: (item) => ({ ...resourceIdentity(item), status: string(item.status) }),
	openCustomKind: (item) => ({ kind: string(item.kind), surface: oneOf(["sidebar", "picker"], item.surface, "a kind surface") }),
	openNamespacePicker: noPayload,
	closeNamespacePicker: noPayload,
	selectAllNamespaces: noPayload,
	clearResourceFilters: noPayload,
	searchNamespacePicker: (item) => ({ value: item.value === "" ? "" : string(item.value) }),
	toggleNamespace: (item) => ({ namespace: string(item.namespace), checked: boolean(item.checked) }),
	expectNamespaceChecked: (item) => ({ namespace: string(item.namespace), checked: boolean(item.checked) }),
	expectNamespaceTrigger: (item) => ({ label: string(item.label) }),
	selectHealthFilter: (item) => ({ label: oneOf(HEALTH_FILTER_LABELS, item.label, "a health filter") }),
	expectHealthFilter: (item) => ({ label: oneOf(HEALTH_FILTER_LABELS, item.label, "a health filter"), pressed: boolean(item.pressed) }),
	chooseGitOpsOwner: (item) => ({ owner: string(item.owner) }),
	setHistoricalReplicaSets: (item) => ({ checked: boolean(item.checked) }),
	expectUsageFooter: (item) => ({ text: string(item.text) }),
	expectUsageProcess: (item) => ({ label: string(item.label) }),
	openUsageTree: noPayload,
	expectNoUsageFooter: noPayload,
	setTopologyMode: (item) => ({ mode: oneOf(["Ownership", "Network Flow"], item.mode, "a topology mode"), pending: optionalBoolean(item.pending) }),
	selectNamespace: (item) => ({ namespace: string(item.namespace) }),
	expectTopology: (item) => ({ nodes: count(item.nodes), edges: count(item.edges) }),
	expectTopologyEdge: (item) => ({ source: string(item.source), target: string(item.target) }),
	expectTopologyEdgeMotion: (item) => ({ source: string(item.source), target: string(item.target), flow: boolean(item.flow), pulse: boolean(item.pulse) }),
	setWindowActive: (item) => ({ active: boolean(item.active) }),
	expectTopologyEmpty: (item) => ({ title: string(item.title) }),
	selectTopologyNode: (item) => ({ id: string(item.id) }),
	expectNoTopologyNode: (item) => ({ id: string(item.id) }),
	expectResourceMetrics: (item) => {
		if (!isString(item.namespace)) throw new Error("Expected namespace");
		return { kind: string(item.kind), namespace: item.namespace, name: string(item.name), cpu: string(item.cpu), memory: string(item.memory) };
	},
	expectMetricsCard: (item) => ({ message: string(item.message), contexts: strings(item.contexts, true) }),
	openResources: noPayload,
	closeDetail: noPayload,
	expectDetailClosed: noPayload,
	expectEmpty: noPayload,
	refresh: noPayload,
	search: (item) => ({ value: item.value === "" ? "" : string(item.value) }),
	expectSearch: (item) => ({ value: item.value === "" ? "" : string(item.value) }),
	expectResource: resourceIdentity,
	expectDetail: resourceIdentity,
	expectClusterResource: clusterResourceIdentity,
	expectNoClusterResource: clusterResourceIdentity,
	expectNoResource: resourceIdentity,
	panelControl: (item) => ({ control: oneOf(PANEL_CONTROLS, item.control, "a panel control") }),
	expectPanelControl: (item) => ({ control: oneOf(PANEL_CONTROLS, item.control, "a panel control"), present: boolean(item.present) }),
	skipGraph: noPayload,
	mapKey: (item) => ({ key: oneOf(MAP_KEYS, item.key, "a graph key") }),
	expectMapFocus: (item) => ({ id: string(item.id) }),
	expectNoMapFocus: (item) => ({ id: string(item.id) }),
	expectMapNodeInView: (item) => {
		if (!isBoolean(item.inView)) throw new Error("Expected a boolean inView");
		return { id: string(item.id), inView: item.inView };
	},
	panMap: (item) => {
		if (!isNumber(item.dx) || !Number.isFinite(item.dx)) throw new Error("Expected a finite dx");
		if (!isNumber(item.dy) || !Number.isFinite(item.dy)) throw new Error("Expected a finite dy");
		return { dx: item.dx, dy: item.dy };
	},
	clickMapControl: (item) => ({ control: oneOf(MAP_CONTROLS, item.control, "a map control") }),
	expectMapControlFocus: (item) => ({ control: oneOf(MAP_CONTROLS, item.control, "a map control") }),
	expectMapZoom: (item) => {
		if (!isNumber(item.zoom) || !Number.isFinite(item.zoom) || item.zoom <= 0) throw new Error("Expected a positive zoom");
		return { zoom: item.zoom };
	},
	resizeDetail: (item) => ({ key: oneOf(DETAIL_RESIZE_KEYS, item.key, "a detail resize key") }),
	expectDetailResize: (item) => ({ value: count(item.value) }),
	sortResourceTable: (item) => {
		return withOptional({
			column: oneOf(SORT_COLUMNS, item.column, "a sortable table column"),
		}, {
			key: item.key !== undefined ? oneOf(["Enter", "Space"] as const, item.key, "a sort key") : undefined,
		});
	},
	expectResourceOrder: (item) => ({ names: strings(item.names, false) }),
	toggleResourceGroup: (item) => {
		return withOptional({
			label: string(item.label),
		}, {
			key: item.key !== undefined ? oneOf(["Enter", "Space"] as const, item.key, "a group key") : undefined,
		});
	},
	expectResourceGroup: (item) => ({ label: string(item.label), expanded: boolean(item.expanded) }),
	openResourceRow: (item) => ({ ...resourceIdentity(item), column: oneOf(TABLE_COLUMNS, item.column, "a table column") }),
	expectResourceCell: (item) => ({ ...resourceIdentity(item), column: oneOf(TABLE_COLUMNS, item.column, "a table column"), text: string(item.text) }),
	expectResourceCellFits: (item) => ({ ...resourceIdentity(item), column: oneOf(TABLE_COLUMNS, item.column, "a table column"), text: string(item.text), truncated: optionalBoolean(item.truncated) ?? true }),
	expectResourceColumns: (item) => ({ columns: strings(item.columns, false).map((column) => oneOf(TABLE_COLUMNS, column, "a table column")) }),
	expectResourceActivity: (item) => ({ ...resourceIdentity(item), state: oneOf(["active", "paused"] as const, item.state, "an activity state") }),
	pressResourceRow: (item) => ({ key: oneOf(["Enter", "Space"], item.key, "Enter or Space"), kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) }),
	openResource: (item) => {
		return withOptional({
			...resourceIdentity(item),
		}, {
			context: item.context !== undefined ? string(item.context) : undefined,
		});
	},
	expectRowContexts: (item) => ({ ...resourceIdentity(item), contexts: strings(item.contexts, false) }),
	filterContexts: (item) => ({ contexts: strings(item.contexts, true) }),
	expectResourceSummary: (item) => ({ text: string(item.text) }),
	selectMapContext: (item) => ({ context: string(item.context) }),
	expectMapContext: (item) => {
		if (!boolean(item.shown)) {
			if (item.context !== undefined) throw new Error("A hidden map context select has no value");
			return { shown: false as const };
		}
		return { shown: true as const, context: string(item.context) };
	},
	mapContextKey: (item) => ({ key: oneOf(MAP_CONTEXT_KEYS, item.key, "a Map context key") }),
	expectMapContextOptions: (item) => {
		return withOptional({
			open: boolean(item.open),
		}, {
			focused: item.focused !== undefined ? boolean(item.focused) : undefined,
		});
	},
} satisfies StepParsers;
