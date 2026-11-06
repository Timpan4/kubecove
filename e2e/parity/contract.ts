import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import { LIVE_SCRIPTS } from "../../src/lib/tauri-dev-mock-live";

import { ARGO_OPTION_NAMES, DETAIL_RESIZE_KEYS, LOG_LEVELS, MAP_CONTROLS, MAP_KEYS, PANEL_CONTROLS, RBAC_CATEGORIES, RBAC_RISKS, SORT_COLUMNS, type Step, TABLE_COLUMNS, VERDICTS } from "./contract-steps";

export type { Step } from "./contract-steps";
function oneOf<T extends string>(values: readonly T[], value: unknown, label: string): T {
	const found = values.find((candidate) => candidate === value);
	if (!found) throw new Error(`Expected ${label}`);
	return found;
}
export type Scenario = { id: string; title: string; steps: Step[] };
export type Contract = { scenarios: Scenario[] };

/** Every action both drivers must implement; `contract.test.ts` checks each driver handles each one. */
export const ACTIONS = [
    "expectOverviewShortcuts", "overviewShortcut", "expectActivePage", "expectGitOpsSelection",
    "expectOverviewScope", "expectCompareTotal",
    "retryOverview", "expectOverviewRetry",
    "dragKind",
    "expectEmptyLauncher", "expectEntryCount", "startWithoutWorkspace", "expectNoWorkspaceShell", "expectPaletteUnavailable",
    "copyErrorDetail",
    "expectForegroundLoading", "expectFriendlyError", "inspectRequiredPermission", "expectPermissionTarget", "returnToSource",
    "copyResourceName", "expectCopyToast", "dismissCopyToast",
    "toggleResourcePin", "expectWorkspaceEntry", "openWorkspaceEntry",
    "reorderKindKey", "resetKindOrder", "expectRemoteHost", "expectRemoteHostTooltip", "expectNoRemoteHost",
    "configureShellRead", "retryLauncher", "expectLauncherRetry",
    "overviewOperation", "overviewIncident", "reviewIncidentAction",
    "expectTopologyDenied",
    "mockTiming",
    "coverageSelect",
    "coverageCopy",
    "coverageScript",
    "coverageCheck",
    "coverageClick", "coverageFill", "coverageText", "paletteKey", "expectPaletteOpen", "deleteSavedWorkspace", "importDecision", "cancelImport", "searchEvents",
    "terminalFit",
    "openNamespaceList", "expectNamespaceRow", "resizeViewport", "openNavigationDrawer", "expectNavigationDrawer", "sidebarKey",
    "terminalOutput", "terminalSelect", "terminalCopy", "terminalPaste", "terminalScroll",
    "configureArgoMock", "argoControl", "focusArgoResource", "expectNoSavedArgoProfile", "copyArgoValue", "configureArgoInventory", "expandArgoDiff", "reconnectArgoProfile",
    "failWorkspaceContext", "expectWorkspaceReadWarning",
    "editSavedWorkspace", "expectNoSavedWorkspace", "saveWorkspace", "cancelWorkspaceEdit", "toggleWorkspaceContext", "expectOverviewHealth", "expectDetailContext",
    "sidebarToggle", "sidebarSelect", "expectSidebarRow", "collapseSidebar", "expectSidebarBranch", "scrollSidebar", "expectSidebarReveal", "refreshPage",
    "replayPageReads", "focusStalePage", "expectPageText",
    "toggleDetailDisclosure", "expectNoPageText",
    "searchCustomKinds", "searchKindPicker", "expectCustomGroup", "toggleKind", "openKindPicker", "closeKindPicker", "selectAllKinds", "expectKindChecked",
    "openNamespacePicker", "closeNamespacePicker", "selectAllNamespaces", "clearResourceFilters", "searchNamespacePicker", "toggleNamespace", "expectNamespaceChecked", "expectNamespaceTrigger",
    "selectHealthFilter", "expectHealthFilter", "chooseGitOpsOwner", "setHistoricalReplicaSets",
    "fillMcpPort", "mcpControl", "expectMcpState", "expectMcpCopy", "expectMcpRequests",
    "selectArgoPreference", "expectSavedArgoProfile", "configureArgoSync", "selectArgoHistory", "expectArgoWorkspaceTransport", "completeArgoOperation", "confirmArgoTunnel",
    "connectArgoProfile", "disconnectArgoProfile", "confirmArgoReview", "cancelArgoReview", "reviewArgoOperation", "expectArgoReview", "expectArgoText", "argoDocument", "expectArgoPopover", "dismissArgoPopover", "expectArgoCheckbox", "toggleArgoOption", "expectNoArgoControl", "expectArgoFocusInPopover", "tabArgoPopover", "expectArgoTriggerFocus", "expectArgoOptionFocusable","expectArgoOperationState","selectArgoConnection",
    "cancelYamlEdit",
    "revealSecretValue", "hideSecretValue", "expectSecretValue", "expectYamlLens", "refreshLatencyReport", "clearLatencyReport", "expectLatencyEmpty", "expectLatencyTiming", "openDocumentation", "expectDocumentationRequest", "copyLatencyReport", "expectLatencyReportCopy", "updateStatusControl",
    "configurePreset", "presetControl",
    "replaySessions", "confirmExecLeave",
    "configureForward", "forwardControl", "expectForwardState", "execControl", "expectExecState",
    "expectWorkspaceExport",
    "fillSettingsField", "clickSettingsButton", "clickKubeconfigPathButton", "expectKubeconfigPathBefore", "expectSettingsText", "expectNoSettingsText", "expectRuntimeEndpoint", "expectMaskedToken", "reopenStoredSettings", "attemptUnverifiedUpdate", "configureSettingsMock",
	"expectLiveState", "replayLiveData", "expectResourceStatus", "openCustomKind",
	"selectTopologyNode", "expectTopologyEmpty", "expectUsageFooter", "openUsageTree", "expectNoUsageFooter", "expectUsageProcess", "setTopologyMode", "selectNamespace", "expectTopology", "expectTopologyEdge", "expectNoTopologyNode", "expectResourceMetrics",
	"expectYamlAbsent",
	"clickActionEntry",
	"fillContainerImage",
	"openActionEditor", "fillActionField", "chooseActionRevision", "selectYamlShape", "selectYamlEncoding", "replaceYamlDraft", "expectYamlDocument", "expectYamlPicker", "formatYaml",
	"openResources", "showYaml", "closeDetail", "expectDetailClosed", "expectEmpty", "refresh", "search", "expectSearch",
	"expectResource", "openResource", "expectDetail", "expectDetailTab", "expectClusterResource", "expectNoClusterResource", "expectYaml", "openPage", "showTab", "filterEvents",
	"expectDetailScroll", "expectEvent", "expectEventCount", "expectLiveSessionCount", "filterIncidents", "expectIncidentGuide", "openIncidentResources", "expectGitOpsCount", "expectPortForward",
	"expectLogLines", "filterLogs", "toggleLogLevel", "expectLogsEmpty", "selectRevision", "expectRevisionDiff", "reviewAction", "editReplicas",
	"toggleLogRow", "toggleLogExpandAll", "expectLogRowOpen", "copyLogLine", "expectRestorePrompt", "restorePromptControl",
	"expectReview", "confirmDeletion", "executeReview", "closeReview", "expectNoReview", "expectActionResult", "expectNoActionResult",
	"editYaml", "applyYaml", "dryRunYaml", "expectDryRun", "expectYamlNotice", "expectNoYamlNotice", "expectYamlReadOnly",
	"expectNoResource", "openLauncher", "openSavedWorkspace", "expectSavedWorkspace", "expectContextUnavailable", "nameWorkspace", "selectWorkspaceContext", "toggleWorkspaceNamespace",
	"createWorkspace", "expectOpenWorkspace", "selectRbacCategory", "expectRbacCategory", "filterRbacRisk", "expectRbacItem", "expectNoRbacItem", "selectRbacItem",
	"investigateRbacSubject", "expectRbacIdentity", "expectRbacGrant", "expectNoRbacGrant", "verifyRbacAccess", "expectRbacVerdict", "openSettings", "closeSettings",
	"showSettingsSection", "searchSettings", "expectSettingsHeading", "expectSettingRow", "expectNoSettingRow", "expectSettingsEmpty", "setKubeconfigEnvVar",
	"expectKubeconfigEnvVar", "addServerKubeconfigPath", "moveResourceKindEarlier", "expectResourceKindPosition", "expectDeliveryView", "restartClient", "toggleSetting", "expectSetting", "selectSettingOption", "expectSettingOption", "openPalette", "searchPalette", "expectPaletteResource", "expectNoPaletteResource",
	"choosePaletteResource", "expectPaletteEntry", "expectNoPaletteEntry", "choosePaletteEntry",
	"editRbacReviewNote", "expectRbacReviewNote", "saveRbacReview", "expectRbacReview", "clearRbacReview",
	"openGitOps", "expandNav", "expectGitOpsTable", "expectGitOpsItem", "expectNoGitOpsItem", "openGitOpsDetails", "expectArgoApplication",
	"expectArgoSignal", "expectHelmCount", "expectHelmRelease", "expectNoHelmRelease", "openHelmRelease", "selectHelmNamespace", "searchHelm",
	"expectHelmDetails", "openGitOpsResources", "expectArgoWorkspace", "filterArgoResources", "setDeliveryView",
    "sortResourceTable", "expectResourceOrder", "toggleResourceGroup", "expectResourceGroup", "openResourceRow", "expectResourceCell",
	"clickDetailControl",
	"openLiveSessionPopover", "expectLiveSessionRow", "liveSessionControl", "expectHealthTooltip", "expectTimestampTooltip", "expectEventTimeTooltip",
    "panelControl", "expectPanelControl", "skipGraph", "mapKey", "expectMapFocus", "expectNoMapFocus", "clickMapControl", "expectMapControlFocus", "expectMapZoom", "resizeDetail", "expectDetailResize",
	"expectYamlDiagnostic", "expectNoYamlDiagnostic", "expectYamlApplyDescription", "expectNoYamlApplyDescription", "expectActionLoadError", "expectNoActionLoadError", "retryActionPermission", "expectActionLock",
] as const satisfies readonly Step["action"][];
type Missing = Exclude<Step["action"], (typeof ACTIONS)[number]>;
const allActionsListed: Missing extends never ? true : Missing = true;
void allActionsListed;

function object(value: unknown): Record<string, unknown> {
	if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Expected object");
	return Object.fromEntries(Object.entries(value));
}
function string(value: unknown): string {
	if (typeof value !== "string" || !value.trim()) throw new Error("Expected nonempty string");
	return value;
}
function count(value: unknown): number {
	if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw new Error("Expected a nonnegative integer");
	return value;
}
function strings(value: unknown, allowEmpty: boolean): string[] {
	if (!Array.isArray(value) || (!allowEmpty && !value.length)) throw new Error("Expected a list of strings");
	return value.map(string);
}
function boolean(value: unknown): boolean {
    if (typeof value !== "boolean") throw new Error("Expected boolean");
    return value;
}
function optionalBoolean(value: unknown): boolean | undefined {
    if (value == null) return undefined;
    if (typeof value !== "boolean") throw new Error("Expected boolean");
    return value;
}
function step(value: unknown): Step {
	const item = object(value);
	switch (item.action) {
        case "sidebarToggle": case "sidebarSelect": return { action: item.action, path: strings(item.path, false) };
        case "expectSidebarRow": return { action: item.action, path: strings(item.path, false), visible: boolean(item.visible), selected: boolean(item.selected) };
        case "collapseSidebar": return { action: item.action };
        case "expectSidebarBranch": return { action: item.action, page: string(item.page), expanded: boolean(item.expanded) };
        case "scrollSidebar": return { action: item.action, to: oneOf(["top", "bottom"], item.to, "a sidebar scroll target") };
        case "expectSidebarReveal": return { action: item.action, path: strings(item.path, false), visible: boolean(item.visible), highlighted: boolean(item.highlighted) };
        case "expectEmptyLauncher": case "startWithoutWorkspace": case "expectNoWorkspaceShell": case "expectPaletteUnavailable": return { action: item.action };
        case "expectOverviewShortcuts": return { action: item.action, labels: strings(item.labels, true) };
        case "overviewShortcut": return { action: item.action, label: string(item.label) };
        case "expectActivePage": return { action: item.action, page: string(item.page) };
        case "expectGitOpsSelection": return { action: item.action, name: string(item.name) };
        case "expectOverviewScope": return { action: item.action, labels: strings(item.labels, false) };
        case "expectCompareTotal": return { action: item.action, title: string(item.title), side: string(item.side), total: count(item.total) };
        case "retryOverview": case "expectOverviewRetry": return { action: item.action };
        case "dragKind": return { action: item.action, kind: string(item.kind), target: string(item.target) };
        case "expectEntryCount": return { action: item.action, panel: oneOf(["Pinned", "Recent"], item.panel, "an entry panel"), count: count(item.count) };
        case "copyErrorDetail": return { action: item.action, error: oneOf(["contexts", "namespaces", "details"], item.error, "an error panel"), expected: string(item.expected) };
        case "expectForegroundLoading": return { action: item.action, active: boolean(item.active) };
        case "expectFriendlyError": {
            const friendly: Extract<Step, { action: "expectFriendlyError" }> = { action: item.action, title: string(item.title) };
            if (item.next !== undefined) friendly.next = string(item.next);
            if (item.summary !== undefined) friendly.summary = string(item.summary);
            return friendly;
        }
        case "inspectRequiredPermission": case "returnToSource": case "dismissCopyToast": return { action: item.action };
        case "expectPermissionTarget": return { action: item.action, verb: string(item.verb), resource: string(item.resource) };
        case "copyResourceName": return { action: item.action, kind: string(item.kind), namespace: string(item.namespace), name: string(item.name) };
        case "expectCopyToast": return { action: item.action, text: string(item.text) };
        case "toggleResourcePin": return { action: item.action, kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) };
        case "expectWorkspaceEntry": case "openWorkspaceEntry": return { action: item.action, panel: oneOf(["Pinned", "Recent"], item.panel, "an entry panel"), label: string(item.label), index: count(item.index), count: count(item.count) };
        case "reorderKindKey": return { action: item.action, kind: string(item.kind), key: oneOf(["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"], item.key, "a reorder key") };
        case "resetKindOrder": return { action: item.action };
        case "expectRemoteHost": return { action: item.action, url: string(item.url) };
        case "expectRemoteHostTooltip": return { action: item.action, value: string(item.value) };
        case "expectNoRemoteHost": return { action: item.action };
        case "configureShellRead": return { action: item.action, phase: oneOf(["ready", "contexts-failed", "contexts-empty", "namespaces-failed", "namespaces-credential-failed", "namespaces-empty", "resources-failed", "primary-context-missing", "kind-missing", "payments-api-removed", "sources-failed"], item.phase, "a shell read phase") };
        case "openNamespaceList": case "openNavigationDrawer": return { action: item.action };
        case "expectNamespaceRow": return { action: item.action, name: string(item.name), age: string(item.age) };
        case "resizeViewport": return { action: item.action, width: count(item.width), height: count(item.height) };
        case "expectNavigationDrawer":
            if (typeof item.open !== "boolean") throw new Error("Expected drawer state");
            return { action: item.action, open: item.open };
        case "sidebarKey": return { action: item.action, path: strings(item.path, false), key: oneOf(["Enter", "Space", "ArrowLeft", "ArrowRight"], item.key, "a sidebar key") };
        case "retryLauncher": case "expectLauncherRetry": return { action: item.action, read: oneOf(["contexts", "namespaces"], item.read, "a launcher read") };
        case "overviewOperation": return { action: item.action, destination: oneOf(["Resources", "Workspaces", "Port Forwards", "Incidents"], item.destination, "an overview operation") };
        case "overviewIncident": return { action: item.action, filter: oneOf(["Degraded", "Needs attention", "Restart evidence"], item.filter, "an incident shortcut") };
        case "reviewIncidentAction": return { action: item.action, index: count(item.index) };
        case "expectTopologyDenied": return { action: item.action };
        case "mockTiming": {
            const control = object(item.control);
            const timing: import("../../src/lib/tauri-dev-mock-timing").TimingControl = {
                operation: oneOf(["hold", "dirty", "contend", "release", "fail", "failOnce", "expectRequests", "expectPending", "expectNoRetry", "expectNoError"], control.operation, "a timing operation"),
                command: string(control.command), context: control.context === "" ? "" : string(control.context),
            };
            if (control.kind !== undefined) timing.kind = string(control.kind);
            if (control.namespace !== undefined) timing.namespace = string(control.namespace);
            if (control.count !== undefined) timing.count = count(control.count);
            return { action: item.action, control: timing };
        }
        case "coverageSelect": return { action: item.action, label: string(item.label), value: string(item.value) };
        case "coverageCopy": return { action: item.action, label: string(item.label), text: string(item.text), match: oneOf(["contains", "exact"], item.match, "a clipboard match") };
        case "coverageScript": return { action: item.action, script: oneOf(["base", "loading", "failed", "empty", "partial", "many", "conflict", "logs-empty", "logs-failed", "logs-options", "logs-long", "logs-stream", "startup-failed", "foreign", "detail-loading", "enrichment-failed", "enrichment-loading", "attention-only", "stop-failed"], item.script, "a coverage script") };
        case "coverageCheck": return { action: item.action, check: string(item.check) };
        case "coverageClick": return { action: item.action, label: string(item.label) };
        case "coverageFill": return { action: item.action, label: string(item.label), value: item.value === "" ? "" : string(item.value) };
        case "coverageText": return { action: item.action, text: string(item.text), visible: boolean(item.visible) };
        case "paletteKey": return { action: item.action, key: oneOf(["ArrowDown", "ArrowUp", "Enter", "Escape"], item.key, "a palette key") };
        case "expectPaletteOpen": return { action: item.action, open: boolean(item.open) };
        case "deleteSavedWorkspace": return { action: item.action, name: string(item.name) };
        case "importDecision": return { action: item.action, choice: oneOf(["Add as new", "Replace", "Skip"], item.choice, "an import decision") };
        case "cancelImport": return { action: item.action };
        case "searchEvents": return { action: item.action, value: item.value === "" ? "" : string(item.value) };
        case "failWorkspaceContext": case "expectWorkspaceReadWarning": return { action: item.action, context: string(item.context) };
        case "editSavedWorkspace": case "expectNoSavedWorkspace": return { action: item.action, name: string(item.name) };
        case "saveWorkspace": case "cancelWorkspaceEdit": return { action: item.action };
        case "toggleWorkspaceContext": case "expectDetailContext": return { action: item.action, context: string(item.context) };
        case "expectOverviewHealth": return { action: item.action, total: count(item.total) };
        case "refreshPage": return { action: item.action, page: oneOf(["RBAC", "Incidents"], item.page, "a refresh button page") };
        case "replayPageReads": return { action: item.action, phase: oneOf(["base", "updated", "failed"], item.phase, "a page read phase") };
        case "focusStalePage": return { action: item.action, page: oneOf(["GitOps", "Helm", "RBAC", "Incidents"], item.page, "a refreshing page") };
        case "expectPageText": return { action: item.action, text: string(item.text) };
        case "toggleDetailDisclosure": return { action: item.action, label: string(item.label) };
        case "expectNoPageText": return { action: item.action, text: string(item.text) };
        case "clickDetailControl": return { action: item.action, label: string(item.label) };
        case "fillMcpPort": return { action: item.action, value: string(item.value) };
        case "mcpControl": return { action: item.action, control: oneOf(["MCP server", "Submit port", "Save", "Copy token", "Copy Claude Code command", "Rotate token"], item.control, "an MCP control") };
        case "expectMcpState":
            if (typeof item.enabled !== "boolean" || typeof item.running !== "boolean" || typeof item.save !== "boolean" || typeof item.command !== "boolean") throw new Error("Expected MCP control states");
            return { action: item.action, enabled: item.enabled, port: string(item.port), running: item.running, save: item.save, command: item.command };
        case "expectMcpCopy":
            if (typeof item.rotated !== "boolean") throw new Error("Expected rotation state");
            return { action: item.action, kind: oneOf(["token", "command"], item.kind, "an MCP copy kind"), rotated: item.rotated };
        case "expectMcpRequests": return { action: item.action, requests: strings(item.requests, true) };
        case "selectArgoPreference": return { action: item.action, preference: oneOf(["Automatic", "Kubernetes"], item.preference, "an inspection preference") };
        case "expectSavedArgoProfile": return { action: item.action, url: string(item.url) };
        case "configureArgoSync": return { action: item.action, revision: string(item.revision), name: string(item.name) };
        case "selectArgoHistory": return { action: item.action, revision: string(item.revision) };
        case "expectArgoWorkspaceTransport": return { action: item.action, text: string(item.text) };
        case "confirmArgoTunnel": return { action: item.action, failure: item.failure == null ? undefined : string(item.failure) };
        case "completeArgoOperation": return { action: item.action };
        case "connectArgoProfile": return { action: item.action, url: string(item.url), remember: optionalBoolean(item.remember), tunnel: optionalBoolean(item.tunnel), local: optionalBoolean(item.local), insecure: optionalBoolean(item.insecure), customCa: item.customCa == null ? undefined : string(item.customCa), surface: item.surface == null ? undefined : oneOf<"page">(["page"], item.surface, "an Argo connection surface") };
        case "disconnectArgoProfile": case "confirmArgoReview": case "cancelArgoReview": return { action: item.action };
        case "configureArgoMock": return { action: item.action, script: oneOf(["base", "discovery-error", "target-mismatch", "preview-error", "snapshot-error", "operation-error", "lazy", "full-diff"], item.script, "an Argo fixture script") };
        case "argoControl": return { action: item.action, control: oneOf(["Open connection", "Close connection", "Forget", "Retry", "Confirm target again", "Retry operation", "Retry state refresh", "Retry snapshot", "Technical detail", "Show diff", "Show fewer lines", "All changes", "Sync options", "Sync with these options", "Refresh options"], item.control, "an Argo control"), url: item.url == null ? undefined : string(item.url) };
        case "focusArgoResource": return { action: item.action, key: string(item.key), label: string(item.label) };
        case "expectNoSavedArgoProfile": return { action: item.action, url: string(item.url) };
        case "expandArgoDiff": return { action: item.action };
        case "reconnectArgoProfile": return { action: item.action };
        case "configureArgoInventory": return { action: item.action, mode: oneOf(["base", "absent", "empty", "partial", "unavailable"], item.mode, "an Argo inventory mode") };
        case "copyArgoValue": return { action: item.action, control: oneOf(["source", "revision", "repository value", "release value", "cluster target value"], item.control, "an Argo copy control"), contains: string(item.contains) };
        case "reviewArgoOperation": return { action: item.action, operation: oneOf(["Refresh", "Hard refresh", "Sync", "Rollback"], item.operation, "an Argo operation") };
        case "expectArgoReview": return { action: item.action, operation: string(item.operation), name: string(item.name), namespace: string(item.namespace), transport: string(item.transport) };
        case "expectArgoText": return { action: item.action, text: string(item.text) };
        case "expectArgoPopover": return { action: item.action, control: oneOf(["Sync options", "Refresh options"], item.control, "an Argo options popover"), open: boolean(item.open) };
        case "dismissArgoPopover": return { action: item.action, how: oneOf(["Escape", "outside click"], item.how, "a popover dismissal") };
        case "expectArgoCheckbox": return { action: item.action, name: oneOf(ARGO_OPTION_NAMES, item.name, "an Argo option"), checked: boolean(item.checked) };
        case "toggleArgoOption": return { action: item.action, name: oneOf(ARGO_OPTION_NAMES, item.name, "an Argo option") };
        case "expectNoArgoControl": return { action: item.action, control: oneOf(["Confirm target again"], item.control, "an Argo control") };
        case "expectArgoFocusInPopover": return { action: item.action, control: oneOf(["Sync options", "Refresh options"], item.control, "an Argo options popover") };
        case "tabArgoPopover": return { action: item.action, control: oneOf(["Sync options", "Refresh options"], item.control, "an Argo options popover"), presses: count(item.presses) };
        case "expectArgoTriggerFocus": return { action: item.action, control: oneOf(["Sync options", "Refresh options"], item.control, "an Argo options trigger") };
        case "expectArgoOptionFocusable": return { action: item.action, name: oneOf(["Prune", "Dry run", "Force / replace"], item.name, "an Argo sync option") };
        case "expectArgoOperationState": return { action: item.action, control: oneOf(["Sync", "Refresh", "Sync options", "Refresh options"], item.control, "an Argo operation control"), enabled: boolean(item.enabled) };
        case "selectArgoConnection": return { action: item.action, url: string(item.url) };
        case "argoDocument": return { action: item.action, view: oneOf(["Desired", "Live", "Changes"], item.view, "an Argo document"), contains: string(item.contains) };
        case "cancelYamlEdit": return { action: item.action };
        case "revealSecretValue": case "hideSecretValue": case "refreshLatencyReport": case "clearLatencyReport": case "expectLatencyEmpty": case "openDocumentation": return { action: item.action };
        case "expectSecretValue":
            if (typeof item.visible !== "boolean") throw new Error("Expected Secret visibility");
            return { action: item.action, visible: item.visible };
        case "expectYamlLens": return { action: item.action, line: count(item.line), ...(item.marked === undefined ? {} : { marked: string(item.marked) }) };
        case "expectYamlDiagnostic":
            if (item.severity !== "error" && item.severity !== "warning" && item.severity !== "info") throw new Error("Expected a diagnostic severity");
            return { action: item.action, severity: item.severity, source: string(item.source), message: string(item.message), path: string(item.path) };
        case "expectNoYamlDiagnostic": return { action: item.action, message: string(item.message) };
        case "expectYamlApplyDescription": return { action: item.action, text: string(item.text) };
        case "expectNoYamlApplyDescription": return { action: item.action };
        case "expectActionLoadError": return { action: item.action, text: string(item.text) };
        case "expectNoActionLoadError": case "retryActionPermission": return { action: item.action };
        case "expectActionLock":
            if (typeof item.locked !== "boolean") throw new Error("Expected a lock state");
            return { action: item.action, row: string(item.row), locked: item.locked, ...(item.reason === undefined ? {} : { reason: string(item.reason) }) };
        case "expectLatencyTiming": return { action: item.action, command: string(item.command) };
        case "copyLatencyReport": return { action: item.action };
        case "expectLatencyReportCopy": return { action: item.action, contains: string(item.contains) };
        case "updateStatusControl": return { action: item.action, control: oneOf(["Open", "Check", "Later", "Escape"], item.control, "an update control") };
        case "expectDocumentationRequest": return { action: item.action, url: string(item.url) };
        case "fillSettingsField": return { action: item.action, field: oneOf(["Server URL", "Server token"], item.field, "a remote field"), value: string(item.value) };
        case "clickSettingsButton": return { action: item.action, label: string(item.label) };
        case "clickKubeconfigPathButton": return { action: item.action, value: string(item.value), label: string(item.label) };
        case "expectKubeconfigPathBefore": return { action: item.action, value: string(item.value), other: string(item.other) };
        case "expectSettingsText": case "expectNoSettingsText": case "expectRuntimeEndpoint": return { action: item.action, value: string(item.value) };
        case "expectMaskedToken": case "reopenStoredSettings": case "attemptUnverifiedUpdate": return { action: item.action };
        case "configureSettingsMock": return { action: item.action, script: oneOf(["choose-kubeconfig", "choose-workspace", "choose-unavailable-workspace", "choose-application-workspace", "cancel-dialog", "no-update", "invalid-signature", "update-available"], item.script, "a settings script") };
        case "terminalFit": return { action: item.action, width: count(item.width), height: count(item.height) };
        case "terminalOutput": return { action: item.action, data: string(item.data), container: string(item.container) };
        case "terminalSelect": return { action: item.action, row: count(item.row), col: count(item.col), endRow: count(item.endRow), endCol: count(item.endCol), word: optionalBoolean(item.word) ?? false };
        case "terminalCopy": case "terminalPaste": return { action: item.action, text: item.text === "" ? "" : string(item.text) };
        case "terminalScroll": return { action: item.action, rows: count(item.rows) };
        case "expectWorkspaceExport": return { action: item.action, name: string(item.name) };
        case "configurePreset": return { action: item.action, label: string(item.label), context: string(item.context), namespace: string(item.namespace), service: string(item.service), remote: string(item.remote), local: string(item.local) };
        case "presetControl": return { action: item.action, operation: oneOf(["add", "save", "edit", "start", "remove", "absent"], item.operation, "a preset action"), ...(item.label === undefined ? {} : { label: string(item.label) }) };
        case "replaySessions": return { action: item.action, script: oneOf(["reconnecting", "failed", "foreign-output"], item.script, "a session script") };
        case "confirmExecLeave": return { action: item.action, choice: oneOf(["stay", "stop"], item.choice, "a leave choice") };
        case "configureForward": return { action: item.action, remote: string(item.remote), local: typeof item.local === "string" ? item.local : "" };
        case "forwardControl": return { action: item.action, operation: oneOf(["start", "save", "restore", "stop", "reconnect"], item.operation, "a forward control"), ...(item.target === undefined ? {} : { target: string(item.target) }) };
        case "expectForwardState": return { action: item.action, target: string(item.target), local: string(item.local), status: string(item.status), count: count(item.count), ...(item.resolved === undefined ? {} : { resolved: string(item.resolved) }) };
        case "execControl": {
            const control: Extract<Step, { action: "execControl" }> = {
                action: item.action,
                operation: oneOf(["start", "input", "close", "stop", "select", "custom", "arguments", "shortcut", "run"], item.operation, "an exec control"),
            };
            if (item.container !== undefined) control.container = string(item.container);
            if (item.command !== undefined) control.command = string(item.command);
            if (item.data !== undefined) control.data = string(item.data);
            if (item.tab !== undefined) control.tab = count(item.tab);
            return control;
        }
        case "expectExecState": return { action: item.action, container: string(item.container), status: string(item.status), contains: typeof item.contains === "string" ? item.contains : "", ...(item.absent === undefined ? {} : { absent: string(item.absent) }), ...(item.command === undefined ? {} : { command: string(item.command) }) };
        case "expectLiveState": return { action: item.action, label: string(item.label), ...(item.notice === undefined ? {} : { notice: string(item.notice) }) };
        case "replayLiveData": return { action: item.action, script: oneOf(LIVE_SCRIPTS, item.script, "a live script") };
        case "expectResourceStatus": return { action: item.action, kind: string(item.kind), namespace: string(item.namespace), name: string(item.name), status: string(item.status) };
        case "openCustomKind": return { action: item.action, kind: string(item.kind), surface: oneOf(["sidebar", "picker"], item.surface, "a kind surface") };
        case "searchCustomKinds": case "searchKindPicker": return { action: item.action, value: item.value === "" ? "" : string(item.value) };
        case "expectCustomGroup": return { action: item.action, group: string(item.group), visible: boolean(item.visible) };
        case "toggleKind": case "expectKindChecked": return { action: item.action, kind: string(item.kind), checked: boolean(item.checked) };
        case "openKindPicker": case "closeKindPicker": case "selectAllKinds": return { action: item.action };
        case "openNamespacePicker": case "closeNamespacePicker": case "selectAllNamespaces": case "clearResourceFilters": return { action: item.action };
        case "searchNamespacePicker": return { action: item.action, value: item.value === "" ? "" : string(item.value) };
        case "toggleNamespace": case "expectNamespaceChecked": return { action: item.action, namespace: string(item.namespace), checked: boolean(item.checked) };
        case "expectNamespaceTrigger": return { action: item.action, label: string(item.label) };
        case "selectHealthFilter": return { action: item.action, label: oneOf(["Total", "Healthy", "Unknown", "Not evaluated", "Needs attention", "Degraded", "Restart evidence"], item.label, "a health filter") };
        case "expectHealthFilter": return { action: item.action, label: oneOf(["Total", "Healthy", "Unknown", "Not evaluated", "Needs attention", "Degraded", "Restart evidence"], item.label, "a health filter"), pressed: boolean(item.pressed) };
        case "chooseGitOpsOwner": return { action: item.action, owner: string(item.owner) };
        case "setHistoricalReplicaSets": return { action: item.action, checked: boolean(item.checked) };
        case "expectUsageFooter": return { action: item.action, text: string(item.text) };
        case "expectUsageProcess": return { action: item.action, label: string(item.label) };
        case "openUsageTree": case "expectNoUsageFooter": return { action: item.action };
        case "setTopologyMode": return { action: item.action, mode: oneOf(["Ownership", "Network Flow"], item.mode, "a topology mode"), pending: optionalBoolean(item.pending) };
        case "selectNamespace": return { action: item.action, namespace: string(item.namespace) };
        case "expectTopology": return { action: item.action, nodes: count(item.nodes), edges: count(item.edges) };
        case "expectTopologyEdge": return { action: item.action, source: string(item.source), target: string(item.target) };
        case "expectTopologyEmpty": return { action: item.action, title: string(item.title) };
        case "selectTopologyNode": case "expectNoTopologyNode": return { action: item.action, id: string(item.id) };
        case "expectResourceMetrics":
            if (typeof item.namespace !== "string") throw new Error("Expected namespace");
            return { action: item.action, kind: string(item.kind), namespace: item.namespace, name: string(item.name), cpu: string(item.cpu), memory: string(item.memory) };
		case "expectYamlAbsent": return { action: item.action, fragments: strings(item.fragments, false) };
		case "clickActionEntry": return { action: item.action, label: string(item.label), button: oneOf(["Add", "Remove", "Restore"], item.button, "an entry action") };
		case "fillContainerImage": return { action: item.action, container: string(item.container), value: string(item.value) };
		case "openActionEditor": return { action: item.action, row: oneOf(["image", "environment", "resources", "labels", "annotations", "rollback"], item.row, "an action editor") };
		case "fillActionField": case "expectYamlPicker": return { action: item.action, label: string(item.label), value: string(item.value) };
		case "chooseActionRevision": return { action: item.action, revision: count(item.revision) };
		case "selectYamlShape": case "selectYamlEncoding": case "replaceYamlDraft": case "expectYamlDocument": return { action: item.action, value: string(item.value) };
		case "formatYaml": return { action: item.action };
		case "openResources": case "showYaml": case "closeDetail": case "expectDetailClosed": case "expectEmpty": case "refresh":
			return { action: item.action };
		case "search": case "expectSearch": return { action: item.action, value: item.value === "" ? "" : string(item.value) };
		case "expectResource": case "openResource": case "expectDetail": return { action: item.action, kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) };
		case "expectClusterResource": case "expectNoClusterResource": return { action: item.action, kind: string(item.kind), name: string(item.name) };
		case "expectYaml":
			if (!Array.isArray(item.contains) || !item.contains.length) throw new Error("Expected YAML fragments");
			return { action: item.action, contains: item.contains.map(string) };
		case "openPage": return { action: item.action, page: string(item.page) };
		case "showTab": case "expectDetailTab": return { action: item.action, tab: string(item.tab) };
		case "filterEvents":
			if (item.filter !== "All" && item.filter !== "Warnings" && item.filter !== "Normal") throw new Error("Expected an event filter");
			return { action: item.action, filter: item.filter };
		case "expectDetailScroll": {
            if (item.view !== "events" && item.view !== "timeline") throw new Error("Expected event or timeline view");
            return { action: item.action, view: item.view };
        }
        case "expectEvent": return { action: item.action, index: count(item.index), reason: string(item.reason), message: string(item.message) };
		case "expectEventCount": case "expectLiveSessionCount": return { action: item.action, count: count(item.count) };
		case "filterIncidents": return { action: item.action, filter: string(item.filter) };
		case "expectIncidentGuide": return { action: item.action, title: string(item.title) };
		case "openIncidentResources": return { action: item.action };
		case "expectGitOpsCount": return { action: item.action, label: string(item.label), count: count(item.count) };
		case "expectPortForward": return { action: item.action, title: string(item.title) };
		case "closeReview": case "expectNoReview": case "executeReview": case "expectNoActionResult": case "editYaml": case "applyYaml": case "dryRunYaml": case "expectYamlReadOnly":
			return { action: item.action };
		// An empty list asserts that no log row renders.
		case "expectLogLines": return { action: item.action, lines: strings(item.lines, true) };
		case "filterLogs": return { action: item.action, value: item.value === "" ? "" : string(item.value) };
		case "confirmDeletion": case "editReplicas": return { action: item.action, value: string(item.value) };
		case "toggleLogLevel": {
			const level = LOG_LEVELS.find((candidate) => candidate === item.level);
			if (!level) throw new Error("Expected a log level");
			return { action: item.action, level };
		}
		case "expectLogsEmpty": case "expectYamlNotice": case "expectNoYamlNotice": return { action: item.action, title: string(item.title) };
		case "toggleLogRow": {
			const row: Extract<Step, { action: "toggleLogRow" }> = { action: item.action, line: string(item.line) };
			if (item.key !== undefined) row.key = oneOf<"Enter" | "Space">(["Enter", "Space"], item.key, "a log row key");
			return row;
		}
		case "toggleLogExpandAll": return { action: item.action };
		case "expectLogRowOpen": return { action: item.action, line: string(item.line), open: boolean(item.open) };
		case "copyLogLine": return { action: item.action, line: string(item.line), contains: string(item.contains) };
		case "expectRestorePrompt": return { action: item.action, visible: boolean(item.visible), ...(item.count === undefined ? {} : { count: count(item.count) }) };
		case "restorePromptControl": return { action: item.action, control: oneOf(["Start saved", "Review", "Skip", "Auto-start"], item.control, "a restore prompt control") };
		case "selectRevision": case "reviewAction": return { action: item.action, label: string(item.label) };
		case "expectRevisionDiff":
			return { action: item.action, selected: string(item.selected), comparison: string(item.comparison), contains: strings(item.contains, false) };
		case "expectReview":
			return { action: item.action, title: string(item.title), description: string(item.description), contains: strings(item.contains, false) };
		case "expectActionResult": return { action: item.action, text: string(item.text) };
		case "expectDryRun": return { action: item.action, contains: strings(item.contains, false) };
		case "expectNoResource":
			return { action: item.action, kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) };
		case "expectPaletteResource": case "expectNoPaletteResource": case "choosePaletteResource":
			if (typeof item.namespace !== "string") throw new Error("Expected a resource namespace, empty for cluster resources");
			return { action: item.action, kind: string(item.kind), name: string(item.name), namespace: item.namespace };
		case "openLauncher": case "createWorkspace": case "openSettings": case "closeSettings": return { action: item.action };
		case "nameWorkspace": case "searchSettings": case "setKubeconfigEnvVar": case "expectKubeconfigEnvVar": case "addServerKubeconfigPath": case "expectSettingsEmpty": case "searchPalette":
			return { action: item.action, value: string(item.value) };
		case "openSavedWorkspace": return { action: item.action, name: string(item.name) };
		case "expectSavedWorkspace": return { action: item.action, name: string(item.name), summary: string(item.summary) };
		case "expectContextUnavailable": return { action: item.action, name: string(item.name) };
		case "selectWorkspaceContext": return { action: item.action, context: string(item.context) };
		case "toggleWorkspaceNamespace": return { action: item.action, namespace: string(item.namespace) };
		case "expectOpenWorkspace": return { action: item.action, name: string(item.name), context: string(item.context) };
		case "selectRbacCategory": case "expectRbacCategory": return { action: item.action, category: oneOf(RBAC_CATEGORIES, item.category, "an RBAC category") };
		case "filterRbacRisk": return { action: item.action, filter: oneOf(RBAC_RISKS, item.filter, "an RBAC risk filter") };
		case "expectRbacItem": return { action: item.action, title: string(item.title), risk: string(item.risk) };
		case "expectNoRbacItem": case "selectRbacItem": return { action: item.action, title: string(item.title) };
		case "investigateRbacSubject": return { action: item.action, subject: string(item.subject) };
		case "expectRbacIdentity": return { action: item.action, identity: string(item.identity) };
		case "expectRbacGrant": case "expectNoRbacGrant": return { action: item.action, grant: string(item.grant) };
		case "verifyRbacAccess": return { action: item.action, verb: string(item.verb) };
		case "expectRbacVerdict": return { action: item.action, outcome: oneOf(VERDICTS, item.outcome, "an access review outcome") };
		case "editRbacReviewNote": case "expectRbacReviewNote":
			if (typeof item.value !== "string") throw new Error("Expected review note text");
			return { action: item.action, value: item.value };
		case "saveRbacReview":
			return { action: item.action, disposition: oneOf(["expected", "anomalous"], item.disposition, "a review disposition") };
		case "expectRbacReview": return { action: item.action, label: string(item.label) };
		case "clearRbacReview": return { action: item.action };
		case "showSettingsSection": return { action: item.action, section: string(item.section) };
		case "expectSettingsHeading": return { action: item.action, heading: string(item.heading) };
		case "expectSettingRow": case "expectNoSettingRow": return { action: item.action, title: string(item.title) };
		case "selectSettingOption": case "expectSettingOption": return { action: item.action, setting: string(item.setting), value: string(item.value) };
        case "moveResourceKindEarlier": return { action: item.action, kind: string(item.kind) };
        case "expectResourceKindPosition": return { action: item.action, kind: string(item.kind), lane: string(item.lane), index: count(item.index) };
        case "expectDeliveryView": return { action: item.action, page: string(item.page), mode: string(item.mode) };
        case "restartClient": return { action: item.action };
        case "toggleSetting": return { action: item.action, setting: string(item.setting) };
		case "expectSetting":
			if (typeof item.checked !== "boolean") throw new Error("Expected a checked state");
			return { action: item.action, setting: string(item.setting), checked: item.checked };
		case "openPalette":
			if (item.via !== "button" && item.via !== "shortcut") throw new Error("Expected how the palette opens");
			return { action: item.action, via: item.via };
		case "expectPaletteEntry": case "expectNoPaletteEntry": case "choosePaletteEntry": return { action: item.action, label: string(item.label) };
		case "openGitOps": return { action: item.action };
		case "openGitOpsResources":
			return { action: item.action, kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) };
		case "expectArgoWorkspace": return { action: item.action, name: string(item.name) };
		case "filterArgoResources": return { action: item.action, filter: string(item.filter) };
		case "setDeliveryView": {
			if (item.page !== "GitOps" && item.page !== "Helm") throw new Error("Expected a delivery page");
			if (item.mode !== "Cards" && item.mode !== "List") throw new Error("Expected Cards or List");
			return { action: item.action, page: item.page, mode: item.mode };
		}
		case "panelControl": return { action: item.action, control: oneOf(PANEL_CONTROLS, item.control, "a panel control") };
		case "expectPanelControl": return { action: item.action, control: oneOf(PANEL_CONTROLS, item.control, "a panel control"), present: boolean(item.present) };
		case "skipGraph": return { action: item.action };
		case "mapKey": return { action: item.action, key: oneOf(MAP_KEYS, item.key, "a graph key") };
		case "expectMapFocus": case "expectNoMapFocus": return { action: item.action, id: string(item.id) };
		case "clickMapControl": case "expectMapControlFocus": return { action: item.action, control: oneOf(MAP_CONTROLS, item.control, "a map control") };
		case "expectMapZoom": {
			if (typeof item.zoom !== "number" || !Number.isFinite(item.zoom) || item.zoom <= 0) throw new Error("Expected a positive zoom");
			return { action: item.action, zoom: item.zoom };
		}
		case "resizeDetail": return { action: item.action, key: oneOf(DETAIL_RESIZE_KEYS, item.key, "a detail resize key") };
		case "expectDetailResize": return { action: item.action, value: count(item.value) };
		case "expandNav": return { action: item.action, page: string(item.page) };
		case "expectGitOpsTable": return { action: item.action, title: string(item.title), count: count(item.count) };
		case "expectGitOpsItem": case "expectNoGitOpsItem": case "openGitOpsDetails":
			return { action: item.action, kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) };
		case "expectArgoApplication": return { action: item.action, name: string(item.name), health: string(item.health), sync: string(item.sync) };
		case "expectArgoSignal": return { action: item.action, label: string(item.label), count: count(item.count) };
		case "expectHelmCount": return { action: item.action, count: count(item.count) };
		case "expectHelmRelease": case "expectNoHelmRelease": case "openHelmRelease":
			return { action: item.action, name: string(item.name), namespace: string(item.namespace) };
		case "selectHelmNamespace": return { action: item.action, namespace: string(item.namespace) };
		case "searchHelm": return { action: item.action, value: string(item.value) };
		case "expectHelmDetails": return { action: item.action, title: string(item.title), description: string(item.description), totals: string(item.totals) };
		case "sortResourceTable": return { action: item.action, column: oneOf(SORT_COLUMNS, item.column, "a sortable table column") };
		case "expectResourceOrder": return { action: item.action, names: strings(item.names, false) };
		case "toggleResourceGroup": return { action: item.action, label: string(item.label) };
		case "expectResourceGroup": return { action: item.action, label: string(item.label), expanded: boolean(item.expanded) };
		case "openResourceRow": return { action: item.action, kind: string(item.kind), name: string(item.name), namespace: string(item.namespace), column: oneOf(TABLE_COLUMNS, item.column, "a table column") };
		case "expectResourceCell": return { action: item.action, kind: string(item.kind), name: string(item.name), namespace: string(item.namespace), column: oneOf(TABLE_COLUMNS, item.column, "a table column"), text: string(item.text) };
		case "openLiveSessionPopover": return { action: item.action };
		case "expectLiveSessionRow": return { action: item.action, target: string(item.target), status: string(item.status), controls: strings(item.controls, false) };
		case "liveSessionControl": return { action: item.action, operation: oneOf(["Show", "Reconnect", "Copy", "Stop", "Manage"], item.operation, "a live-session control"), ...(item.target === undefined ? {} : { target: string(item.target) }) };
		case "expectHealthTooltip": return { action: item.action, kind: string(item.kind), namespace: string(item.namespace), name: string(item.name), state: string(item.state), source: string(item.source), statuses: strings(item.statuses, true), partial: boolean(item.partial) };
		case "expectTimestampTooltip": return { action: item.action, kind: string(item.kind), namespace: string(item.namespace), name: string(item.name), value: string(item.value) };
		case "expectEventTimeTooltip": return { action: item.action, index: count(item.index) };
		default: throw new Error(`Unknown action: ${String(item.action)}`);
	}
}
export function parseContract(value: unknown): Contract {
	const input = object(value);
	if (input.fixture !== "browser-mocks" || input.context !== "mock-dev" || !Array.isArray(input.scenarios) || !input.scenarios.length) throw new Error("Invalid parity contract");
	const ids = new Set<string>();
	return { scenarios: input.scenarios.map((value: unknown) => {
		const scenario = object(value);
		const id = string(scenario.id);
		if (ids.has(id)) throw new Error(`Duplicate scenario: ${id}`);
		ids.add(id);
		if (!Array.isArray(scenario.steps) || !scenario.steps.length) throw new Error(`Missing steps: ${id}`);
		return { id, title: string(scenario.title), steps: scenario.steps.map(step) };
	}) };
}

export const SCENARIO_DIR = "e2e/parity/scenarios";

/** Reads every `*.json` scenario file in name order, as the GPUI driver does. IDs must be unique across files. */
export async function loadContract(directory = SCENARIO_DIR): Promise<Contract> {
	const files = (await readdir(directory)).filter((name) => name.endsWith(".json")).sort();
	if (!files.length) throw new Error(`No scenario files in ${directory}`);
	const scenarios: Scenario[] = [];
	const ids = new Set<string>();
	for (const file of files) {
		for (const scenario of parseContract(JSON.parse(await readFile(join(directory, file), "utf8"))).scenarios) {
			if (ids.has(scenario.id)) throw new Error(`Duplicate scenario: ${scenario.id} in ${file}`);
			ids.add(scenario.id);
			scenarios.push(scenario);
		}
	}
	return { scenarios };
}

/** The equal-tests rule: every view the GPUI client renders needs a scenario whose ID starts with `<view>.`. */
export function requireViewCoverage(contract: Contract, views: readonly string[]): void {
	const missing = views.filter((view) => !contract.scenarios.some((scenario) => scenario.id.startsWith(`${view}.`)));
	if (missing.length) throw new Error(`Views without shared parity scenarios: ${missing.join(", ")}`);
}

export function validateReports(contract: Contract, reports: unknown[]): void {
	const drivers = new Set<string>();
	for (const value of reports) {
		const report = object(value);
		const driver = string(report.driver);
		if ((driver !== "web" && driver !== "gpui") || drivers.has(driver)) throw new Error(`Unexpected or duplicate driver: ${driver}`);
		drivers.add(driver);
		if (!Array.isArray(report.scenarios)) throw new Error(`Missing scenarios: ${driver}`);
		const expected = new Set(contract.scenarios.map((scenario) => scenario.id));
		for (const value of report.scenarios) {
			const result = object(value);
			const id = string(result.id);
			if (!expected.delete(id) || result.status !== "passed") throw new Error(`Invalid ${driver} result: ${id}`);
		}
		if (expected.size) throw new Error(`Missing ${driver} results: ${[...expected].join(", ")}`);
	}
	if (drivers.size !== 2) throw new Error("Missing driver report");
}
