import type { JsonObject } from "../../src/lib/types";

type Identity = { kind: string; name: string; namespace: string };
/** Checkbox labels inside the Argo Sync options popover. */
export const ARGO_OPTION_NAMES = ["Prune", "Dry run", "Force / replace"] as const;
type ArgoOptionName = (typeof ARGO_OPTION_NAMES)[number];
// Context shows only when the workspace has more than one context.
export const TABLE_COLUMNS = ["Name", "Status", "Context", "Namespace", "Kind", "Ready", "Restarts", "CPU", "Memory", "Owner", "Age"] as const;
type TableColumn = (typeof TABLE_COLUMNS)[number];
export const SORT_COLUMNS = ["Name", "Status", "Context", "Namespace", "Kind", "Ready", "Restarts", "CPU", "Memory", "Age"] as const;
export type UpdateControlLabel = "Check for updates" | "Update available" | "Checking for updates" | "Update check failed" | "Downloading update" | "Restart to finish update";
/** One diagnostic the YAML editor marks. Positions are 1-based `[line, column]` in UTF-16 columns; `to` is exclusive. Without `message`, the clients' texts may differ. */
export type YamlEditorDiagnostic = { from: [number, number]; to: [number, number]; severity: "error" | "warning" | "info"; source: string; message?: string };
export type Step =
    | { action: "expectDetailScroll"; view: "events" | "timeline" }
    | { action: "coverageSelect"; label: string; value: string }
    | { action: "coverageCopy"; label: string; text: string; match: "contains" | "exact" }
    | { action: "coverageScript"; script: "base" | "loading" | "failed" | "empty" | "partial" | "many" | "conflict" | "logs-empty" | "logs-failed" | "logs-options" | "logs-long" | "logs-stream" | "startup-failed" | "foreign" | "detail-loading" | "enrichment-failed" | "enrichment-loading" | "attention-only" | "stop-failed" | "stop-pending" }
    | { action: "coverageCheck"; check: string }
    | { action: "coverageClick"; label: string }
    | { action: "coverageFill"; label: string; value: string }
    | { action: "coverageText"; text: string; visible: boolean }
    | { action: "paletteKey"; key: "ArrowDown" | "ArrowUp" | "Enter" | "Escape" }
    | { action: "expectPaletteOpen"; open: boolean }
    | { action: "deleteSavedWorkspace"; name: string }
    | { action: "importDecision"; choice: "Add as new" | "Replace" | "Skip" }
    | { action: "cancelImport" }
    | { action: "searchEvents"; value: string }
    | { action: "sidebarToggle" | "sidebarSelect"; path: string[] }
    | { action: "expectSidebarRow"; path: string[]; visible: boolean; selected: boolean }
    | { action: "collapseSidebar" }
    | { action: "expectSidebarBranch"; page: string; expanded: boolean }
    | { action: "scrollSidebar"; to: "top" | "bottom" }
    | { action: "expectSidebarReveal"; path: string[]; visible: boolean; highlighted: boolean }
    | { action: "expectOverviewShortcuts"; labels: string[] }
    | { action: "overviewShortcut"; label: string }
    | { action: "expectOverviewScope"; labels: string[] }
    | { action: "expectCompareTotal"; title: string; side: string; total: number }
    | { action: "retryOverview" | "expectOverviewRetry" }
    | { action: "dragKind"; kind: string; target: string }
    | { action: "expectEmptyLauncher" }
    | { action: "startWithoutWorkspace" | "expectNoWorkspaceShell" | "expectPaletteUnavailable" }
    | { action: "expectEntryCount"; panel: "Pinned" | "Recent"; count: number }
    | { action: "copyErrorDetail"; error: "contexts" | "namespaces" | "details"; expected: string }
    | { action: "expectForegroundLoading"; active: boolean }
    | { action: "expectFriendlyError"; title: string; next?: string; summary?: string }
    | { action: "inspectRequiredPermission" | "returnToSource" | "dismissCopyToast" }
    | { action: "expectPermissionTarget"; verb: string; resource: string }
    | ({ action: "copyResourceName" } & Identity)
    | { action: "expectCopyToast"; text: string }
    /** `context` names the row's context in a multi-context workspace, where each context has its own row. */
    | ({ action: "toggleResourcePin"; context?: string } & Identity)
    | { action: "expectWorkspaceEntry" | "openWorkspaceEntry"; panel: "Pinned" | "Recent"; label: string; index: number; count: number }
    | { action: "reorderKindKey"; kind: string; key: "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" }
    | { action: "resetKindOrder" }
    | { action: "expectRemoteHost"; url: string; disconnected?: boolean }
    | { action: "expectRemoteHostTooltip"; value: string }
    | { action: "expectNoRemoteHost" }
    | { action: "dropRemoteConnection" }
    | { action: "remoteSessionSignal"; state: "reconnecting" | "sessionEnded" | "signInExpired" | "sessionExpired" }
    | { action: "expectRemoteNotice"; value: string }
    | { action: "expectNoRemoteNotice" }
    | { action: "clickRemoteNoticeButton"; label: "Sign in" | "Copy sign-in link" | "Cancel" | "Start a new session" | "Disconnect" }
    | { action: "configureShellRead"; phase: "ready" | "contexts-failed" | "contexts-empty" | "namespaces-failed" | "namespaces-credential-failed" | "namespaces-empty" | "resources-failed" | "primary-context-missing" | "kind-missing" | "payments-api-removed" | "payments-deployments-forbidden" | "payments-ingress-version-changed" | "sources-failed" }
    | { action: "openNamespaceList" }
    | { action: "expectNamespaceRow"; name: string; age: string }
    | { action: "resizeViewport"; width: number; height: number }
    | { action: "openNavigationDrawer" }
    | { action: "expectNavigationDrawer"; open: boolean }
    | { action: "sidebarKey"; path: string[]; key: "Enter" | "Space" | "ArrowLeft" | "ArrowRight" }
    | { action: "retryLauncher"; read: "contexts" | "namespaces" }
    | { action: "expectLauncherRetry"; read: "contexts" | "namespaces" }
    | { action: "overviewOperation"; destination: "Resources" | "Workspaces" | "Port Forwards" | "Incidents" }
    | { action: "overviewIncident"; filter: "Degraded" | "Needs attention" | "Restart evidence" }
    | { action: "reviewIncidentAction"; index: number }
    | { action: "expectTopologyDenied" }
    | { action: "expectWatchStart"; kind: string; resourceVersion: string | null }
    | { action: "mockTiming"; control: import("../../src/lib/tauri-dev-mock-timing").TimingControl }
    | { action: "terminalFit"; width: number; height: number }
    | { action: "terminalOutput"; data: string; container: string }
    // `word` double-clicks, `line` triple-clicks; `alt` drags a rectangle or, for a click, moves the cursor.
    | { action: "terminalSelect"; row: number; col: number; endRow: number; endCol: number; word: boolean; line: boolean; alt: boolean }
    | { action: "terminalCopy" | "terminalPaste" | "terminalPrimaryPaste"; text: string }
    | { action: "terminalScroll"; rows: number }
    // Press at a cell and hold the pointer past the rows' top or bottom edge; `terminalRelease` lets go after `ms`.
    | { action: "terminalDragOutside"; row: number; col: number; edge: "above" | "below" }
    | { action: "terminalRelease"; ms: number }
    | { action: "terminalFocus"; focused: boolean }
    // A right press keeps an existing selection: xterm ignores button 2 while text is selected.
    | { action: "terminalRightClick"; row: number; col: number }
    // `cursor` is the shape with " blink" when it blinks, or "outline" without focus.
    | { action: "expectTerminalCursor"; cursor: string; row?: number; col?: number }
    | { action: "failWorkspaceContext" | "expectWorkspaceReadWarning"; context: string }
    // A held context's scope reads answer only once `releaseWorkspaceContext` names it.
    | { action: "holdWorkspaceContext" | "releaseWorkspaceContext"; context: string }
    // The contexts the "Waiting for context" notice names; empty means no notice.
    | { action: "expectContextsLoading"; contexts: string[] }
    | { action: "editSavedWorkspace" | "expectNoSavedWorkspace"; name: string }
    | { action: "saveWorkspace" | "cancelWorkspaceEdit" }
    | { action: "toggleWorkspaceContext"; context: string }
    | { action: "expectOverviewHealth"; total: number }
    | { action: "expectActivePage"; page: string }
    | { action: "expectGitOpsSelection"; name: string }
    | { action: "expectDetailContext"; context: string }
    | { action: "refreshPage"; page: "RBAC" | "Incidents" }
    | { action: "replayPageReads"; phase: "base" | "updated" | "failed" }
    | { action: "focusStalePage"; page: "GitOps" | "Helm" | "RBAC" | "Incidents" }
    | { action: "expectPageText"; text: string }
    | { action: "toggleDetailDisclosure"; label: string }
    | { action: "expectNoPageText"; text: string }
    | { action: "controlClock" }
    | { action: "advanceClock"; ms: number }
    | { action: "searchCustomKinds" | "searchKindPicker"; value: string }
    | { action: "expectCustomGroup"; group: string; visible: boolean }
    | { action: "toggleKind"; kind: string; checked: boolean }
    | { action: "openKindPicker" | "closeKindPicker" | "selectAllKinds" }
    | { action: "openNamespacePicker" | "closeNamespacePicker" | "selectAllNamespaces" | "clearResourceFilters" }
    | { action: "searchNamespacePicker"; value: string }
    | { action: "toggleNamespace" | "expectNamespaceChecked"; namespace: string; checked: boolean }
    | { action: "expectNamespaceTrigger"; label: string }
    | { action: "selectHealthFilter"; label: string }
    | { action: "expectHealthFilter"; label: string; pressed: boolean }
    | { action: "chooseGitOpsOwner"; owner: string }
    | { action: "setHistoricalReplicaSets"; checked: boolean }
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
    | { action: "completeArgoOperation" }
    | { action: "confirmArgoTunnel"; failure?: string }
    | { action: "connectArgoProfile"; url: string; surface?: "page"; remember?: boolean; tunnel?: boolean; local?: boolean; insecure?: boolean; customCa?: string }
    | { action: "disconnectArgoProfile" | "confirmArgoReview" | "cancelArgoReview" }
    | { action: "reviewArgoOperation"; operation: "Refresh" | "Hard refresh" | "Sync" | "Rollback" }
    | { action: "configureArgoMock"; script: import("../../src/lib/tauri-dev-mock-argo").ArgoMockScript }
    | { action: "argoControl"; control: "Open connection" | "Close connection" | "Disconnect" | "Forget" | "Retry" | "Confirm target again" | "Retry operation" | "Retry state refresh" | "Retry snapshot" | "Technical detail" | "Show diff" | "Show fewer lines" | "All changes" | "Sync options" | "Sync with these options" | "Refresh options"; url?: string }
    | { action: "focusArgoResource"; key: string; label: string }
    | { action: "expectNoSavedArgoProfile"; url: string }
    | { action: "expandArgoDiff" }
    | { action: "reconnectArgoProfile" }
    | { action: "configureArgoInventory"; mode: import("../../src/lib/tauri-dev-mock-argo").ArgoInventoryMode }
    | { action: "copyArgoValue"; control: "source" | "revision" | "repository value" | "release value" | "cluster target value"; contains: string }
    | { action: "expectArgoReview"; operation: string; name: string; namespace: string; transport: string }
    | { action: "expectArgoText"; text: string }
    | { action: "expectArgoPopover"; control: "Sync options" | "Refresh options"; open: boolean }
    | { action: "dismissArgoPopover"; how: "Escape" | "outside click" }
    | { action: "expectArgoCheckbox"; name: ArgoOptionName; checked: boolean }
    | { action: "toggleArgoOption"; name: ArgoOptionName }
    | { action: "expectNoArgoControl"; control: "Confirm target again" | "Reconnect" }
    | { action: "expectArgoFocusInPopover"; control: "Sync options" | "Refresh options" }
    | { action: "tabArgoPopover"; control: "Sync options" | "Refresh options"; presses: number }
    | { action: "argoConnectionDialog"; operation: "open" | "expectFocusIn" | "tabCycle" | "shiftTabCycle" | "escape" | "outsideClick" | "expectOpen" | "expectConnectForm" | "expectRemember" | "expectLocalLogin" | "expectInsecure" | "chooseLocalLogin" | "chooseInsecure" | "openDetail" | "chooseFoundInCluster" | "chooseExternalUrl" | "expectFoundInCluster" | "expectInWindow" | "expectReachable"; open: boolean; checked: boolean; control: string }
    | { action: "fillArgoConnectionField" | "expectArgoConnectionField"; field: "url" | "rootPath" | "tlsServerName" | "token" | "password"; value: string }
    | { action: "expectArgoTriggerFocus"; control: "Sync options" | "Refresh options" }
    | { action: "expectArgoOptionFocusable"; name: "Prune" | "Dry run" | "Force / replace" }
    | { action: "expectArgoOperationState"; control: "Sync" | "Refresh" | "Sync options" | "Refresh options"; enabled: boolean }
    | { action: "selectArgoConnection"; url: string }
    | { action: "argoDocument"; view: "Desired" | "Live" | "Changes"; contains: string }
    | { action: "cancelYamlEdit" }
    | { action: "revealSecretValue" | "hideSecretValue" | "refreshLatencyReport" | "clearLatencyReport" | "expectLatencyEmpty" | "openDocumentation" }
    | { action: "expectSecretValue"; visible: boolean }
    | { action: "expectYamlLens"; line: number; marked?: string }
    | { action: "expectLatencyTiming"; command: string }
    | { action: "expectDocumentationRequest"; url: string }
    | { action: "copyLatencyReport" }
    | { action: "expectLatencyReportCopy"; contains: string }
    | { action: "expectViewTiming"; view: "resources" }
    | { action: "copyInteractions" | "clearInteractions" }
    | { action: "expectInteractionCopy"; contains?: string; excludes?: string }
    | { action: "typeSettingsField"; field: "Server token"; value: string }
    | { action: "runTopologySpike" }
    | { action: "expectTopologySpikeText"; value: string }
    | { action: "updateStatusControl"; control: "Open" | "Check" | "Later" | "Install" | "Relaunch" | "Escape" }
    | { action: "expectUpdateControl"; label: UpdateControlLabel }
    | { action: "configurePreset"; label: string; context: string; namespace: string; service: string; remote: string; local: string }
    | { action: "presetControl"; operation: "add" | "save" | "edit" | "start" | "remove" | "absent"; label?: string }
    | { action: "replaySessions"; script: "reconnecting" | "failed" | "foreign-output" | "exec-output" | "exec-exited" | "exec-failed" | "exec-stopped" }
    | { action: "confirmExecLeave"; choice: "stay" | "stop" }
    | { action: "configureForward"; remote: string; local: string }
    | { action: "forwardControl"; operation: "start" | "save" | "restore" | "stop" | "reconnect"; target?: string }
    | { action: "expectForwardState"; target: string; local: string; status: string; resolved?: string; count: number }
    /** The Forward tab shows one neutral notice with this title and text, and no error. */
    | { action: "expectForwardNotice"; title: string; text: string }
    | { action: "execControl"; operation: "start" | "input" | "close" | "stop" | "select" | "custom" | "arguments" | "shortcut" | "run"; container?: string; command?: string; data?: string; tab?: number }
    | { action: "expectExecState"; container: string; status: string; contains: string; absent?: string; command?: string }
    | { action: "expectWorkspaceExport" | "exportSavedWorkspace"; name: string }
    | { action: "expectExportedShortcut"; name: string; shortcut: JsonObject }
    | { action: "fillSettingsField"; field: "Server URL" | "Server token"; value: string }
    | { action: "clickSettingsButton"; label: string }
    | { action: "clickKubeconfigPathButton"; value: string; label: string }
    | { action: "expectKubeconfigPathBefore"; value: string; other: string }
    | { action: "expectAccessibleName"; name: string }
    | { action: "expectSettingsText" | "expectNoSettingsText" | "expectRuntimeEndpoint"; value: string }
    | { action: "expectMaskedToken" | "reopenStoredSettings" }
    | { action: "configureSettingsMock"; script: "oidc-server" | "oidc-browser-waits" | "oidc-sign-in-fails" | "oidc-memory-only" | "oidc-store-keeps-sign-in" | "access-server" | "choose-kubeconfig" | "choose-workspace" | "choose-unavailable-workspace" | "choose-application-workspace" | "choose-compare-workspace" | "choose-last-export" | "cancel-dialog" | "no-update" | "invalid-signature" | "update-available" | "update-installs" | "update-download-waits" }
    | { action: "expectLiveState"; label: string; notice?: string }
    /** Opens the live-updates menu and checks its status message, refresh hint and refresh button. */
    | { action: "expectLiveMenu"; message: string }
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
    | { action: "foldYamlLine"; line: number }
    | { action: "expectYamlVisibleLines"; lines: number[] }
    | { action: "expectYamlEditorDiagnostics"; diagnostics: YamlEditorDiagnostic[] }
    | { action: "openResources" | "showYaml" | "closeDetail" | "expectDetailClosed" | "expectEmpty" | "refresh" }
    | { action: "search" | "expectSearch"; value: string }
    | ({ action: "expectResource" | "expectDetail" } & Identity)
    /** `context` picks the row of that context when several contexts list the same object. */
    | ({ action: "openResource"; context?: string } & Identity)
    /** The contexts of every row with this identity, in display order. */
    | ({ action: "expectRowContexts"; contexts: string[] } & Identity)
    /** Sets the Context filter to exactly these contexts. An empty list unchecks every context, which clears the filter. */
    | { action: "filterContexts"; contexts: string[] }
    /** The Resource Table status line, such as "14 resources · 2 contexts · page 1 of 1". */
    | { action: "expectResourceSummary"; text: string }
    | { action: "selectMapContext"; context: string }
    /** The ownership map's "Map context" select and its value; `shown: false` asserts there is no select. */
    | { action: "expectMapContext"; shown: true; context: string }
    | { action: "expectMapContext"; shown: false }
    | { action: "expectClusterResource" | "expectNoClusterResource"; kind: string; name: string }
    | { action: "expectYaml"; contains: string[] }
    | { action: "openPage"; page: string }
    | { action: "showTab" | "expectDetailTab"; tab: string }
    | { action: "filterEvents"; filter: "All" | "Warnings" | "Normal" }
    | { action: "expectEvent"; index: number; reason: string; message: string }
    | { action: "expectEventCount" | "expectLiveSessionCount"; count: number }
    | { action: "filterIncidents"; filter: string }
    | { action: "expectIncidentGuide"; title: string }
    | { action: "openIncidentResources" }
    | { action: "expectGitOpsCount"; label: string; count: number }
    | { action: "expectPortForward"; title: string }
    | { action: "closeReview" | "expectNoReview" | "executeReview" | "expectNoActionResult" | "editYaml" | "applyYaml" | "dryRunYaml" | "expectYamlReadOnly" }
    | { action: "expectLogLines"; lines: string[] }
    | { action: "expectLogToolbarFit"; state: "Live" | "Paused" }
    | { action: "filterLogs" | "confirmDeletion" | "editReplicas"; value: string }
    | { action: "toggleLogLevel"; level: LogLevel }
    | { action: "toggleLogRow"; line: string; key?: "Enter" | "Space" }
    | { action: "toggleLogExpandAll" }
    | { action: "expectLogRowOpen"; line: string; open: boolean }
    | { action: "copyLogLine"; line: string; contains: string }
    | { action: "expectLogTimestampTooltip"; line: string; value: string }
    | { action: "expectRestorePrompt"; visible: boolean; count?: number }
    | { action: "restorePromptControl"; control: "Start saved" | "Review" | "Skip" | "Auto-start" }
    | { action: "expectLogsEmpty" | "expectYamlNotice" | "expectNoYamlNotice"; title: string }
    | { action: "selectRevision" | "reviewAction"; label: string }
    | { action: "expectRevisionDiff"; selected: string; comparison: string; contains: string[] }
    | { action: "expectReview"; title: string; description: string; contains: string[] }
    // Without `key` the disclosure is clicked; with it, focused and activated from the keyboard.
    | { action: "showReviewDiff"; key?: "Enter" | "Space" }
    // `controller` is "" when the target has no controlling owner; `typed` says whether the name input shows.
    | { action: "expectDeleteReview"; name: string; context: string; namespace: string; controller: string; consequence: string; typed: boolean }
    | { action: "expectActionResult"; text: string }
    | { action: "expectDryRun"; contains: string[] }
    | ({ action: "expectNoResource" | "expectPaletteResource" | "expectNoPaletteResource" | "choosePaletteResource" } & Identity)
    | { action: "openLauncher" | "createWorkspace" | "openSettings" | "closeSettings" }
    | { action: "nameWorkspace" | "searchSettings" | "setKubeconfigEnvVar" | "expectKubeconfigEnvVar" | "addServerKubeconfigPath" | "expectSettingsEmpty" | "searchPalette"; value: string }
    | { action: "openSavedWorkspace"; name: string }
    | { action: "expectSavedWorkspace"; name: string; summary: string }
    | { action: "expectContextUnavailable"; name: string }
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
    | { action: "setTopologyMode"; mode: "Ownership" | "Network Flow"; pending?: boolean }
    | { action: "selectNamespace"; namespace: string }
    | { action: "expectTopology"; nodes: number; edges: number }
    | { action: "expectTopologyEdge"; source: string; target: string }
    | { action: "expectTopologyEmpty"; title: string }
    | { action: "selectTopologyNode" | "expectNoTopologyNode"; id: string }
    | { action: "expectResourceMetrics"; kind: string; namespace: string; name: string; cpu: string; memory: string }
    | { action: "sortResourceTable"; column: (typeof SORT_COLUMNS)[number] }
    | { action: "expectResourceOrder"; names: string[] }
    | { action: "toggleResourceGroup"; label: string }
    | { action: "expectResourceGroup"; label: string; expanded: boolean }
    | ({ action: "openResourceRow"; column: TableColumn } & Identity)
    | ({ action: "pressResourceRow"; key: "Enter" | "Space" } & Identity)
    | ({ action: "expectResourceCell"; column: TableColumn; text: string } & Identity)
    /** `truncated` defaults to true in the parser. `false` asserts the full value fits. */
    | ({ action: "expectResourceCellFits"; column: TableColumn; text: string; truncated: boolean } & Identity)
    /** The page's column headers, in order, without the unlabeled pin column. */
    | { action: "expectResourceColumns"; columns: TableColumn[] }
    /** A progress lifecycle's activity icon: `active` animates, `paused` holds the last observed state. */
    | ({ action: "expectResourceActivity"; state: "active" | "paused" } & Identity)
    | { action: "setDeliveryView"; page: "GitOps" | "Helm"; mode: "Cards" | "List" }
    | { action: "clickDetailControl"; label: string }
    | { action: "openLiveSessionPopover" }
    | { action: "expectLiveSessionRow"; target: string; status: string; controls: string[] }
    | { action: "liveSessionControl"; operation: "Show" | "Reconnect" | "Copy" | "Stop" | "Manage"; target?: string }
    | ({ action: "expectHealthTooltip" } & Identity & { state: string; source: string; statuses: string[]; partial: boolean })
    | ({ action: "expectTimestampTooltip" } & Identity & { value: string })
    | { action: "expectEventTimeTooltip"; index: number }
    | { action: "panelControl"; control: PanelControl }
    | { action: "expectPanelControl"; control: PanelControl; present: boolean }
    | { action: "skipGraph" }
    | { action: "mapKey"; key: MapKey }
    | { action: "expectMapFocus" | "expectNoMapFocus"; id: string }
    | { action: "expectMapNodeInView"; id: string; inView: boolean }
    | { action: "panMap"; dx: number; dy: number }
    | { action: "clickMapControl" | "expectMapControlFocus"; control: MapControl }
    | { action: "expectMapZoom"; zoom: number }
    | { action: "resizeDetail"; key: DetailResizeKey }
    | { action: "expectDetailResize"; value: number }
    | { action: "expectYamlDiagnostic"; severity: "error" | "warning" | "info"; source: string; message: string; path: string }
    | { action: "expectNoYamlDiagnostic"; message: string }
    | { action: "expectYamlApplyDescription"; text: string }
    | { action: "expectNoYamlApplyDescription" }
    | { action: "expectActionLoadError"; text: string }
    | { action: "expectNoActionLoadError" | "retryActionPermission" }
    | { action: "expectActionLock"; row: string; locked: boolean; reason?: string };
export const PANEL_CONTROLS = [
    "Collapse ownership map", "Show ownership map", "Collapse resource table", "Show resource table",
] as const;
type PanelControl = (typeof PANEL_CONTROLS)[number];
export const MAP_KEYS = ["Tab", "Shift+Tab", "Enter", "Space"] as const;
type MapKey = (typeof MAP_KEYS)[number];
/** The zoom controls, labeled as xyflow labels them on the web. */
export const MAP_CONTROLS = ["Zoom In", "Zoom Out", "Fit View"] as const;
type MapControl = (typeof MAP_CONTROLS)[number];
export const DETAIL_RESIZE_KEYS = ["ArrowLeft", "ArrowRight", "Home", "End"] as const;
type DetailResizeKey = (typeof DETAIL_RESIZE_KEYS)[number];
export const RBAC_CATEGORIES = ["Namespace Access", "Roles", "Cluster Roles", "Bindings", "Service Accounts"] as const;
type RbacCategory = (typeof RBAC_CATEGORIES)[number];
export const RBAC_RISKS = ["All", "High", "Medium", "Low", "No flags", "Unknown"] as const;
type RbacRisk = (typeof RBAC_RISKS)[number];
export const VERDICTS = ["allowed", "denied", "noOpinion"] as const;
type LogLevel = "ERR" | "WRN" | "INF" | "DBG";
export const LOG_LEVELS: readonly LogLevel[] = ["ERR", "WRN", "INF", "DBG"];
