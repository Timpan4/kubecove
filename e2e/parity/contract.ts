import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

type Identity = { kind: string; name: string; namespace: string };
export type Step =
    | { action: "expectOverviewShortcuts"; labels: string[] }
    | { action: "overviewShortcut"; label: string }
    | { action: "expectOverviewScope"; labels: string[] }
    | { action: "expectCompareTotal"; title: string; side: string; total: number }
    | { action: "retryOverview" | "expectOverviewRetry" }
    | { action: "dragKind"; kind: string; target: string }
    | { action: "expectEmptyLauncher" }
    | { action: "expectEntryCount"; panel: "Pinned" | "Recent"; count: number }
    | { action: "copyErrorDetail"; error: "contexts" | "namespaces"; expected: string }
    | ({ action: "toggleResourcePin" } & Identity)
    | { action: "expectWorkspaceEntry" | "openWorkspaceEntry"; panel: "Pinned" | "Recent"; label: string; index: number; count: number }
    | { action: "reorderKindKey"; kind: string; key: "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" }
    | { action: "expectRemoteHost"; url: string }
    | { action: "configureShellRead"; phase: "ready" | "contexts-failed" | "contexts-empty" | "namespaces-failed" | "resources-failed" }
    | { action: "retryLauncher"; read: "contexts" | "namespaces" }
    | { action: "expectLauncherRetry"; read: "contexts" | "namespaces" }
    | { action: "overviewOperation"; destination: "Resources" | "Workspaces" | "Port Forwards" | "Incidents" }
    | { action: "overviewIncident"; filter: "Degraded" | "Needs attention" | "Restart evidence" }
    | { action: "reviewIncidentAction"; index: number }
    | { action: "cancelNextTopology" | "expectCancelledReadRecovery" }
    | { action: "terminalFit"; width: number; height: number }
    | { action: "terminalOutput"; data: string; container: string }
    | { action: "terminalSelect"; row: number; col: number; endRow: number; endCol: number; word: boolean }
    | { action: "terminalCopy" | "terminalPaste"; text: string }
    | { action: "terminalScroll"; rows: number }
    | { action: "failWorkspaceContext" | "expectWorkspaceReadWarning"; context: string }
    | { action: "editSavedWorkspace" | "expectNoSavedWorkspace"; name: string }
    | { action: "saveWorkspace" | "cancelWorkspaceEdit" }
    | { action: "toggleWorkspaceContext"; context: string }
    | { action: "expectOverviewHealth"; total: number }
    | { action: "expectDetailContext"; context: string }
    | { action: "refreshPage"; page: "RBAC" | "Incidents" }
    | { action: "replayPageReads"; phase: "base" | "updated" | "failed" }
    | { action: "focusStalePage"; page: "GitOps" | "Helm" | "RBAC" | "Incidents" }
    | { action: "expectPageText"; text: string }
    | { action: "searchCustomKinds" | "searchKindPicker"; value: string }
    | { action: "expectCustomGroup"; group: string; visible: boolean }
    | { action: "toggleKind"; kind: string; checked: boolean }
    | { action: "openKindPicker" | "closeKindPicker" | "selectAllKinds" }
    | { action: "expectKindChecked"; kind: string; checked: boolean }
    | { action: "fillMcpPort"; value: string }
    | { action: "mcpControl"; control: "MCP server" | "Submit port" | "Save" | "Copy token" | "Copy Claude Code command" | "Rotate token" }
    | { action: "expectMcpState"; enabled: boolean; port: string; running: boolean; save: boolean; command: boolean }
    | { action: "expectMcpCopy"; kind: "token" | "command"; rotated: boolean }
    | { action: "expectMcpRequests"; requests: string[] }
    | { action: "selectArgoPreference"; preference: "Automatic" | "Kubernetes" }
    | { action: "expectSavedArgoProfile"; url: string }
    | { action: "configureArgoSync"; revision: string; name: string }
    | { action: "selectArgoHistory"; revision: string }
    | { action: "expectArgoWorkspaceTransport"; text: string }
    | { action: "completeArgoOperation" | "confirmArgoTunnel" }
    | { action: "connectArgoProfile"; url: string; surface?: "page"; remember?: boolean; tunnel?: boolean; local?: boolean; insecure?: boolean; customCa?: string }
    | { action: "disconnectArgoProfile" | "confirmArgoReview" | "cancelArgoReview" }
    | { action: "reviewArgoOperation"; operation: "Refresh" | "Hard refresh" | "Sync" | "Rollback" }
    | { action: "configureArgoMock"; script: import("../../src/lib/tauri-dev-mock-argo").ArgoMockScript }
    | { action: "argoControl"; control: "Open connection" | "Close connection" | "Forget" | "Retry" | "Confirm target again" | "Retry operation" | "Retry state refresh" | "Retry snapshot" | "Show diff" | "Show fewer lines" | "All changes"; url?: string }
    | { action: "focusArgoResource"; key: string; label: string }
    | { action: "expectNoSavedArgoProfile"; url: string }
    | { action: "expandArgoDiff" }
    | { action: "reconnectArgoProfile" }
    | { action: "configureArgoInventory"; mode: import("../../src/lib/tauri-dev-mock-argo").ArgoInventoryMode }
    | { action: "copyArgoValue"; control: "source" | "revision" | "repository value" | "release value" | "cluster target value"; contains: string }
    | { action: "expectArgoReview"; operation: string; name: string; namespace: string; transport: string }
    | { action: "expectArgoText"; text: string }
    | { action: "argoDocument"; view: "Desired" | "Live" | "Changes"; contains: string }
    | { action: "cancelYamlEdit" }
    | { action: "revealSecretValue" | "hideSecretValue" | "refreshLatencyReport" | "clearLatencyReport" | "expectLatencyEmpty" | "openDocumentation" }
    | { action: "expectSecretValue"; visible: boolean }
    | { action: "expectYamlLens"; line: number }
    | { action: "expectLatencyTiming"; command: string }
    | { action: "expectDocumentationRequest"; url: string }
    | { action: "configurePreset"; label: string; context: string; namespace: string; service: string; remote: string; local: string }
    | { action: "presetControl"; operation: "add" | "save" | "edit" | "start" | "remove" | "absent"; label?: string }
    | { action: "replaySessions"; script: "reconnecting" | "failed" | "foreign-output" }
    | { action: "confirmExecLeave"; choice: "stay" | "stop" }
    | { action: "configureForward"; remote: string; local: string }
    | { action: "forwardControl"; operation: "start" | "save" | "restore" | "stop" | "reconnect"; target?: string }
    | { action: "expectForwardState"; target: string; local: string; status: string; resolved?: string; count: number }
    | { action: "execControl"; operation: "start" | "input" | "close" | "stop" | "select" | "custom" | "arguments" | "run"; container?: string; command?: string; data?: string; tab?: number }
    | { action: "expectExecState"; container: string; status: string; contains: string; absent?: string; command?: string }
    | { action: "expectWorkspaceExport"; name: string }
    | { action: "fillSettingsField"; field: "Server URL" | "Server token"; value: string }
    | { action: "clickSettingsButton"; label: string }
    | { action: "expectSettingsText" | "expectNoSettingsText" | "expectRuntimeEndpoint"; value: string }
    | { action: "expectMaskedToken" | "reopenStoredSettings" | "attemptUnverifiedUpdate" }
    | { action: "configureSettingsMock"; script: "choose-kubeconfig" | "choose-workspace" | "choose-unavailable-workspace" | "cancel-dialog" | "no-update" | "invalid-signature" }
	| { action: "expectLiveState"; label: string; notice?: string }
	| { action: "replayLiveData"; script: import("../../src/lib/tauri-dev-mock-live").LiveScript }
	| ({ action: "expectResourceStatus"; status: string } & Identity)
	| { action: "openCustomKind"; kind: string; surface: "sidebar" | "picker" }
	| { action: "expectYamlAbsent"; fragments: string[] }
	| { action: "clickActionEntry"; label: string; button: "Add" | "Remove" | "Restore" }
	| { action: "fillContainerImage"; container: string; value: string }
	| { action: "openActionEditor"; row: "image" | "environment" | "resources" | "labels" | "annotations" | "rollback" }
	| { action: "fillActionField"; label: string; value: string }
	| { action: "chooseActionRevision"; revision: number }
	| { action: "selectYamlShape" | "selectYamlEncoding" | "replaceYamlDraft" | "expectYamlDocument"; value: string }
	| { action: "expectYamlPicker"; label: string; value: string }
	| { action: "formatYaml" }
	| { action: "openResources" | "showYaml" | "closeDetail" | "expectDetailClosed" | "expectEmpty" | "refresh" }
	| { action: "search" | "expectSearch"; value: string }
	| ({ action: "expectResource" | "openResource" | "expectDetail" } & Identity)
	| { action: "expectClusterResource" | "expectNoClusterResource"; kind: string; name: string }
	| { action: "expectYaml"; contains: string[] }
	| { action: "openPage"; page: string }
	| { action: "showTab"; tab: string }
	| { action: "filterEvents"; filter: "All" | "Warnings" | "Normal" }
	| { action: "expectEvent"; index: number; reason: string; message: string }
	| { action: "expectEventCount" | "expectLiveSessionCount"; count: number }
	| { action: "filterIncidents"; filter: string }
	| { action: "expectIncidentGuide"; title: string }
	| { action: "expectGitOpsCount"; label: string; count: number }
	| { action: "expectPortForward"; title: string }
	| { action: "closeReview" | "expectNoReview" | "executeReview" | "expectNoActionResult" | "editYaml" | "applyYaml" | "dryRunYaml" | "expectYamlReadOnly" }
	| { action: "expectLogLines"; lines: string[] }
	| { action: "filterLogs" | "confirmDeletion" | "editReplicas"; value: string }
	| { action: "toggleLogLevel"; level: LogLevel }
	| { action: "expectLogsEmpty" | "expectYamlNotice" | "expectNoYamlNotice"; title: string }
	| { action: "selectRevision" | "reviewAction"; label: string }
	| { action: "expectRevisionDiff"; selected: string; comparison: string; contains: string[] }
	| { action: "expectReview"; title: string; description: string; contains: string[] }
	| { action: "expectActionResult"; text: string }
	| { action: "expectDryRun"; contains: string[] }
	| ({ action: "expectNoResource" | "expectPaletteResource" | "expectNoPaletteResource" | "choosePaletteResource" } & Identity)
	| { action: "openLauncher" | "createWorkspace" | "openSettings" | "closeSettings" }
	| { action: "nameWorkspace" | "searchSettings" | "setKubeconfigEnvVar" | "expectKubeconfigEnvVar" | "expectSettingsEmpty" | "searchPalette"; value: string }
	| { action: "openSavedWorkspace"; name: string }
	| { action: "expectSavedWorkspace"; name: string; summary: string }
	| { action: "selectWorkspaceContext"; context: string }
	| { action: "toggleWorkspaceNamespace"; namespace: string }
	| { action: "expectOpenWorkspace"; name: string; context: string }
	| { action: "selectRbacCategory" | "expectRbacCategory"; category: RbacCategory }
	| { action: "filterRbacRisk"; filter: RbacRisk }
	| { action: "expectRbacItem"; title: string; risk: string }
	| { action: "expectNoRbacItem" | "selectRbacItem"; title: string }
	| { action: "investigateRbacSubject"; subject: string }
	| { action: "expectRbacIdentity"; identity: string }
	| { action: "expectRbacGrant" | "expectNoRbacGrant"; grant: string }
	| { action: "verifyRbacAccess"; verb: string }
	| { action: "expectRbacVerdict"; outcome: "allowed" | "denied" | "noOpinion" }
	| { action: "editRbacReviewNote" | "expectRbacReviewNote"; value: string }
	| { action: "saveRbacReview"; disposition: "expected" | "anomalous" }
	| { action: "expectRbacReview"; label: string }
	| { action: "clearRbacReview" }
	| { action: "showSettingsSection"; section: string }
	| { action: "expectSettingsHeading"; heading: string }
	| { action: "expectSettingRow" | "expectNoSettingRow"; title: string }
	| { action: "restartClient" }
    | { action: "expectDeliveryView"; page: string; mode: string }
    | { action: "moveResourceKindEarlier"; kind: string }
    | { action: "expectResourceKindPosition"; kind: string; lane: string; index: number }
    | { action: "toggleSetting"; setting: string }
    | { action: "selectSettingOption" | "expectSettingOption"; setting: string; value: string }
	| { action: "expectSetting"; setting: string; checked: boolean }
	| { action: "openPalette"; via: "button" | "shortcut" }
	| { action: "expectPaletteEntry" | "expectNoPaletteEntry" | "choosePaletteEntry"; label: string }
	| { action: "openGitOps" }
	| { action: "expandNav"; page: string }
	| { action: "expectGitOpsTable"; title: string; count: number }
	| ({ action: "expectGitOpsItem" | "expectNoGitOpsItem" | "openGitOpsDetails" } & Identity)
	| { action: "expectArgoApplication"; name: string; health: string; sync: string }
	| { action: "expectArgoSignal"; label: string; count: number }
	| { action: "expectHelmCount"; count: number }
	| { action: "expectHelmRelease" | "expectNoHelmRelease" | "openHelmRelease"; name: string; namespace: string }
	| { action: "selectHelmNamespace"; namespace: string }
	| { action: "searchHelm"; value: string }
	| { action: "expectHelmDetails"; title: string; description: string; totals: string }
	| ({ action: "openGitOpsResources" } & Identity)
	| { action: "expectArgoWorkspace"; name: string }
	| { action: "filterArgoResources"; filter: string }
	| { action: "expectUsageFooter"; text: string }
    | { action: "openUsageTree" | "expectNoUsageFooter" }
    | { action: "expectUsageProcess"; label: string }
    | { action: "setTopologyMode"; mode: "Ownership" | "Network Flow" }
    | { action: "selectNamespace"; namespace: string }
    | { action: "expectTopology"; nodes: number; edges: number }
    | { action: "expectTopologyEdge"; source: string; target: string }
    | { action: "expectTopologyEmpty"; title: string }
    | { action: "selectTopologyNode" | "expectNoTopologyNode"; id: string }
    | { action: "expectResourceMetrics"; kind: string; namespace: string; name: string; cpu: string; memory: string }
    | { action: "setDeliveryView"; page: "GitOps" | "Helm"; mode: "Cards" | "List" };
const RBAC_CATEGORIES = ["Namespace Access", "Roles", "Cluster Roles", "Bindings", "Service Accounts"] as const;
type RbacCategory = (typeof RBAC_CATEGORIES)[number];
const RBAC_RISKS = ["All", "High", "Medium", "Low", "No flags", "Unknown"] as const;
type RbacRisk = (typeof RBAC_RISKS)[number];
const VERDICTS = ["allowed", "denied", "noOpinion"] as const;
function oneOf<T extends string>(values: readonly T[], value: unknown, label: string): T {
	const found = values.find((candidate) => candidate === value);
	if (!found) throw new Error(`Expected ${label}`);
	return found;
}
type LogLevel = "ERR" | "WRN" | "INF" | "DBG";
const LOG_LEVELS: readonly LogLevel[] = ["ERR", "WRN", "INF", "DBG"];
export type Scenario = { id: string; title: string; steps: Step[] };
export type Contract = { scenarios: Scenario[] };

/** Every action both drivers must implement; `contract.test.ts` checks each driver handles each one. */
export const ACTIONS = [
    "expectOverviewShortcuts", "overviewShortcut",
    "expectOverviewScope", "expectCompareTotal",
    "retryOverview", "expectOverviewRetry",
    "dragKind",
    "expectEmptyLauncher", "expectEntryCount",
    "copyErrorDetail",
    "toggleResourcePin", "expectWorkspaceEntry", "openWorkspaceEntry",
    "reorderKindKey", "expectRemoteHost",
    "configureShellRead", "retryLauncher", "expectLauncherRetry",
    "overviewOperation", "overviewIncident", "reviewIncidentAction",
    "cancelNextTopology", "expectCancelledReadRecovery",
    "terminalFit",
    "terminalOutput", "terminalSelect", "terminalCopy", "terminalPaste", "terminalScroll",
    "configureArgoMock", "argoControl", "focusArgoResource", "expectNoSavedArgoProfile", "copyArgoValue", "configureArgoInventory", "expandArgoDiff", "reconnectArgoProfile",
    "failWorkspaceContext", "expectWorkspaceReadWarning",
    "editSavedWorkspace", "expectNoSavedWorkspace", "saveWorkspace", "cancelWorkspaceEdit", "toggleWorkspaceContext", "expectOverviewHealth", "expectDetailContext",
    "refreshPage",
    "replayPageReads", "focusStalePage", "expectPageText",
    "searchCustomKinds", "searchKindPicker", "expectCustomGroup", "toggleKind", "openKindPicker", "closeKindPicker", "selectAllKinds", "expectKindChecked",
    "fillMcpPort", "mcpControl", "expectMcpState", "expectMcpCopy", "expectMcpRequests",
    "selectArgoPreference", "expectSavedArgoProfile", "configureArgoSync", "selectArgoHistory", "expectArgoWorkspaceTransport", "completeArgoOperation", "confirmArgoTunnel",
    "connectArgoProfile", "disconnectArgoProfile", "confirmArgoReview", "cancelArgoReview", "reviewArgoOperation", "expectArgoReview", "expectArgoText", "argoDocument",
    "cancelYamlEdit",
    "revealSecretValue", "hideSecretValue", "expectSecretValue", "expectYamlLens", "refreshLatencyReport", "clearLatencyReport", "expectLatencyEmpty", "expectLatencyTiming", "openDocumentation", "expectDocumentationRequest",
    "configurePreset", "presetControl",
    "replaySessions", "confirmExecLeave",
    "configureForward", "forwardControl", "expectForwardState", "execControl", "expectExecState",
    "expectWorkspaceExport",
    "fillSettingsField", "clickSettingsButton", "expectSettingsText", "expectNoSettingsText", "expectRuntimeEndpoint", "expectMaskedToken", "reopenStoredSettings", "attemptUnverifiedUpdate", "configureSettingsMock",
	"expectLiveState", "replayLiveData", "expectResourceStatus", "openCustomKind",
	"selectTopologyNode", "expectTopologyEmpty", "expectUsageFooter", "openUsageTree", "expectNoUsageFooter", "expectUsageProcess", "setTopologyMode", "selectNamespace", "expectTopology", "expectTopologyEdge", "expectNoTopologyNode", "expectResourceMetrics",
	"expectYamlAbsent",
	"clickActionEntry",
	"fillContainerImage",
	"openActionEditor", "fillActionField", "chooseActionRevision", "selectYamlShape", "selectYamlEncoding", "replaceYamlDraft", "expectYamlDocument", "expectYamlPicker", "formatYaml",
	"openResources", "showYaml", "closeDetail", "expectDetailClosed", "expectEmpty", "refresh", "search", "expectSearch",
	"expectResource", "openResource", "expectDetail", "expectClusterResource", "expectNoClusterResource", "expectYaml", "openPage", "showTab", "filterEvents",
	"expectEvent", "expectEventCount", "expectLiveSessionCount", "filterIncidents", "expectIncidentGuide", "expectGitOpsCount", "expectPortForward",
	"expectLogLines", "filterLogs", "toggleLogLevel", "expectLogsEmpty", "selectRevision", "expectRevisionDiff", "reviewAction", "editReplicas",
	"expectReview", "confirmDeletion", "executeReview", "closeReview", "expectNoReview", "expectActionResult", "expectNoActionResult",
	"editYaml", "applyYaml", "dryRunYaml", "expectDryRun", "expectYamlNotice", "expectNoYamlNotice", "expectYamlReadOnly",
	"expectNoResource", "openLauncher", "openSavedWorkspace", "expectSavedWorkspace", "nameWorkspace", "selectWorkspaceContext", "toggleWorkspaceNamespace",
	"createWorkspace", "expectOpenWorkspace", "selectRbacCategory", "expectRbacCategory", "filterRbacRisk", "expectRbacItem", "expectNoRbacItem", "selectRbacItem",
	"investigateRbacSubject", "expectRbacIdentity", "expectRbacGrant", "expectNoRbacGrant", "verifyRbacAccess", "expectRbacVerdict", "openSettings", "closeSettings",
	"showSettingsSection", "searchSettings", "expectSettingsHeading", "expectSettingRow", "expectNoSettingRow", "expectSettingsEmpty", "setKubeconfigEnvVar",
	"expectKubeconfigEnvVar", "moveResourceKindEarlier", "expectResourceKindPosition", "expectDeliveryView", "restartClient", "toggleSetting", "expectSetting", "selectSettingOption", "expectSettingOption", "openPalette", "searchPalette", "expectPaletteResource", "expectNoPaletteResource",
	"choosePaletteResource", "expectPaletteEntry", "expectNoPaletteEntry", "choosePaletteEntry",
	"editRbacReviewNote", "expectRbacReviewNote", "saveRbacReview", "expectRbacReview", "clearRbacReview",
	"openGitOps", "expandNav", "expectGitOpsTable", "expectGitOpsItem", "expectNoGitOpsItem", "openGitOpsDetails", "expectArgoApplication",
	"expectArgoSignal", "expectHelmCount", "expectHelmRelease", "expectNoHelmRelease", "openHelmRelease", "selectHelmNamespace", "searchHelm",
	"expectHelmDetails", "openGitOpsResources", "expectArgoWorkspace", "filterArgoResources", "setDeliveryView",
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
        case "expectEmptyLauncher": return { action: item.action };
        case "expectOverviewShortcuts": return { action: item.action, labels: strings(item.labels, true) };
        case "overviewShortcut": return { action: item.action, label: string(item.label) };
        case "expectOverviewScope": return { action: item.action, labels: strings(item.labels, false) };
        case "expectCompareTotal": return { action: item.action, title: string(item.title), side: string(item.side), total: count(item.total) };
        case "retryOverview": case "expectOverviewRetry": return { action: item.action };
        case "dragKind": return { action: item.action, kind: string(item.kind), target: string(item.target) };
        case "expectEntryCount": return { action: item.action, panel: oneOf(["Pinned", "Recent"], item.panel, "an entry panel"), count: count(item.count) };
        case "copyErrorDetail": return { action: item.action, error: oneOf(["contexts", "namespaces"], item.error, "a launcher error"), expected: string(item.expected) };
        case "toggleResourcePin": return { action: item.action, kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) };
        case "expectWorkspaceEntry": case "openWorkspaceEntry": return { action: item.action, panel: oneOf(["Pinned", "Recent"], item.panel, "an entry panel"), label: string(item.label), index: count(item.index), count: count(item.count) };
        case "reorderKindKey": return { action: item.action, kind: string(item.kind), key: oneOf(["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"], item.key, "a reorder key") };
        case "expectRemoteHost": return { action: item.action, url: string(item.url) };
        case "configureShellRead": return { action: item.action, phase: oneOf(["ready", "contexts-failed", "contexts-empty", "namespaces-failed", "resources-failed"], item.phase, "a shell read phase") };
        case "retryLauncher": case "expectLauncherRetry": return { action: item.action, read: oneOf(["contexts", "namespaces"], item.read, "a launcher read") };
        case "overviewOperation": return { action: item.action, destination: oneOf(["Resources", "Workspaces", "Port Forwards", "Incidents"], item.destination, "an overview operation") };
        case "overviewIncident": return { action: item.action, filter: oneOf(["Degraded", "Needs attention", "Restart evidence"], item.filter, "an incident shortcut") };
        case "reviewIncidentAction": return { action: item.action, index: count(item.index) };
        case "failWorkspaceContext": case "expectWorkspaceReadWarning": return { action: item.action, context: string(item.context) };
        case "editSavedWorkspace": case "expectNoSavedWorkspace": return { action: item.action, name: string(item.name) };
        case "saveWorkspace": case "cancelWorkspaceEdit": return { action: item.action };
        case "toggleWorkspaceContext": case "expectDetailContext": return { action: item.action, context: string(item.context) };
        case "expectOverviewHealth": return { action: item.action, total: count(item.total) };
        case "refreshPage": return { action: item.action, page: oneOf(["RBAC", "Incidents"], item.page, "a refresh button page") };
        case "replayPageReads": return { action: item.action, phase: oneOf(["base", "updated", "failed"], item.phase, "a page read phase") };
        case "focusStalePage": return { action: item.action, page: oneOf(["GitOps", "Helm", "RBAC", "Incidents"], item.page, "a refreshing page") };
        case "expectPageText": return { action: item.action, text: string(item.text) };
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
        case "confirmArgoTunnel": case "completeArgoOperation": return { action: item.action };
        case "connectArgoProfile": return { action: item.action, url: string(item.url), remember: optionalBoolean(item.remember), tunnel: optionalBoolean(item.tunnel), local: optionalBoolean(item.local), insecure: optionalBoolean(item.insecure), customCa: item.customCa == null ? undefined : string(item.customCa), surface: item.surface == null ? undefined : oneOf<"page">(["page"], item.surface, "an Argo connection surface") };
        case "disconnectArgoProfile": case "confirmArgoReview": case "cancelArgoReview": return { action: item.action };
        case "configureArgoMock": return { action: item.action, script: oneOf(["base", "discovery-error", "target-mismatch", "snapshot-error", "operation-error", "lazy", "full-diff"], item.script, "an Argo fixture script") };
        case "argoControl": return { action: item.action, control: oneOf(["Open connection", "Close connection", "Forget", "Retry", "Confirm target again", "Retry operation", "Retry state refresh", "Retry snapshot", "Show diff", "Show fewer lines", "All changes"], item.control, "an Argo control"), url: item.url == null ? undefined : string(item.url) };
        case "focusArgoResource": return { action: item.action, key: string(item.key), label: string(item.label) };
        case "expectNoSavedArgoProfile": return { action: item.action, url: string(item.url) };
        case "expandArgoDiff": return { action: item.action };
        case "reconnectArgoProfile": return { action: item.action };
        case "configureArgoInventory": return { action: item.action, mode: oneOf(["base", "absent", "empty", "partial", "unavailable"], item.mode, "an Argo inventory mode") };
        case "copyArgoValue": return { action: item.action, control: oneOf(["source", "revision", "repository value", "release value", "cluster target value"], item.control, "an Argo copy control"), contains: string(item.contains) };
        case "reviewArgoOperation": return { action: item.action, operation: oneOf(["Refresh", "Hard refresh", "Sync", "Rollback"], item.operation, "an Argo operation") };
        case "expectArgoReview": return { action: item.action, operation: string(item.operation), name: string(item.name), namespace: string(item.namespace), transport: string(item.transport) };
        case "expectArgoText": return { action: item.action, text: string(item.text) };
        case "argoDocument": return { action: item.action, view: oneOf(["Desired", "Live", "Changes"], item.view, "an Argo document"), contains: string(item.contains) };
        case "cancelYamlEdit": return { action: item.action };
        case "revealSecretValue": case "hideSecretValue": case "refreshLatencyReport": case "clearLatencyReport": case "expectLatencyEmpty": case "openDocumentation": return { action: item.action };
        case "expectSecretValue":
            if (typeof item.visible !== "boolean") throw new Error("Expected Secret visibility");
            return { action: item.action, visible: item.visible };
        case "expectYamlLens": return { action: item.action, line: count(item.line) };
        case "expectLatencyTiming": return { action: item.action, command: string(item.command) };
        case "expectDocumentationRequest": return { action: item.action, url: string(item.url) };
        case "fillSettingsField": return { action: item.action, field: oneOf(["Server URL", "Server token"], item.field, "a remote field"), value: string(item.value) };
        case "clickSettingsButton": return { action: item.action, label: string(item.label) };
        case "expectSettingsText": case "expectNoSettingsText": case "expectRuntimeEndpoint": return { action: item.action, value: string(item.value) };
        case "expectMaskedToken": case "reopenStoredSettings": case "attemptUnverifiedUpdate": return { action: item.action };
        case "configureSettingsMock": return { action: item.action, script: oneOf(["choose-kubeconfig", "choose-workspace", "choose-unavailable-workspace", "cancel-dialog", "no-update", "invalid-signature"], item.script, "a settings script") };
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
                operation: oneOf(["start", "input", "close", "stop", "select", "custom", "arguments", "run"], item.operation, "an exec control"),
            };
            if (item.container !== undefined) control.container = string(item.container);
            if (item.command !== undefined) control.command = string(item.command);
            if (item.data !== undefined) control.data = string(item.data);
            if (item.tab !== undefined) control.tab = count(item.tab);
            return control;
        }
        case "expectExecState": return { action: item.action, container: string(item.container), status: string(item.status), contains: typeof item.contains === "string" ? item.contains : "", ...(item.absent === undefined ? {} : { absent: string(item.absent) }), ...(item.command === undefined ? {} : { command: string(item.command) }) };
        case "expectLiveState": return { action: item.action, label: string(item.label), ...(item.notice === undefined ? {} : { notice: string(item.notice) }) };
        case "replayLiveData": return { action: item.action, script: oneOf(["update", "delete", "reconnecting", "connected", "events", "forbidden", "failed"], item.script, "a live script") };
        case "expectResourceStatus": return { action: item.action, kind: string(item.kind), namespace: string(item.namespace), name: string(item.name), status: string(item.status) };
        case "openCustomKind": return { action: item.action, kind: string(item.kind), surface: oneOf(["sidebar", "picker"], item.surface, "a kind surface") };
        case "searchCustomKinds": case "searchKindPicker": return { action: item.action, value: item.value === "" ? "" : string(item.value) };
        case "expectCustomGroup": return { action: item.action, group: string(item.group), visible: boolean(item.visible) };
        case "toggleKind": case "expectKindChecked": return { action: item.action, kind: string(item.kind), checked: boolean(item.checked) };
        case "openKindPicker": case "closeKindPicker": case "selectAllKinds": return { action: item.action };
        case "expectUsageFooter": return { action: item.action, text: string(item.text) };
        case "expectUsageProcess": return { action: item.action, label: string(item.label) };
        case "openUsageTree": case "expectNoUsageFooter": return { action: item.action };
        case "setTopologyMode": return { action: item.action, mode: oneOf(["Ownership", "Network Flow"], item.mode, "a topology mode") };
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
		case "search": case "expectSearch": return { action: item.action, value: string(item.value) };
		case "expectResource": case "openResource": case "expectDetail": return { action: item.action, kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) };
		case "expectClusterResource": case "expectNoClusterResource": return { action: item.action, kind: string(item.kind), name: string(item.name) };
		case "expectYaml":
			if (!Array.isArray(item.contains) || !item.contains.length) throw new Error("Expected YAML fragments");
			return { action: item.action, contains: item.contains.map(string) };
		case "openPage": return { action: item.action, page: string(item.page) };
		case "showTab": return { action: item.action, tab: string(item.tab) };
		case "filterEvents":
			if (item.filter !== "All" && item.filter !== "Warnings" && item.filter !== "Normal") throw new Error("Expected an event filter");
			return { action: item.action, filter: item.filter };
		case "expectEvent": return { action: item.action, index: count(item.index), reason: string(item.reason), message: string(item.message) };
		case "expectEventCount": case "expectLiveSessionCount": return { action: item.action, count: count(item.count) };
		case "filterIncidents": return { action: item.action, filter: string(item.filter) };
		case "expectIncidentGuide": return { action: item.action, title: string(item.title) };
		case "expectGitOpsCount": return { action: item.action, label: string(item.label), count: count(item.count) };
		case "expectPortForward": return { action: item.action, title: string(item.title) };
		case "closeReview": case "expectNoReview": case "executeReview": case "expectNoActionResult": case "editYaml": case "applyYaml": case "dryRunYaml": case "expectYamlReadOnly":
			return { action: item.action };
		// An empty list asserts that no log row renders.
		case "expectLogLines": return { action: item.action, lines: strings(item.lines, true) };
		case "filterLogs": case "confirmDeletion": case "editReplicas": return { action: item.action, value: string(item.value) };
		case "toggleLogLevel": {
			const level = LOG_LEVELS.find((candidate) => candidate === item.level);
			if (!level) throw new Error("Expected a log level");
			return { action: item.action, level };
		}
		case "expectLogsEmpty": case "expectYamlNotice": case "expectNoYamlNotice": return { action: item.action, title: string(item.title) };
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
		case "nameWorkspace": case "searchSettings": case "setKubeconfigEnvVar": case "expectKubeconfigEnvVar": case "expectSettingsEmpty": case "searchPalette":
			return { action: item.action, value: string(item.value) };
		case "openSavedWorkspace": return { action: item.action, name: string(item.name) };
		case "expectSavedWorkspace": return { action: item.action, name: string(item.name), summary: string(item.summary) };
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
        case "cancelNextTopology": case "expectCancelledReadRecovery": return { action: item.action };
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
