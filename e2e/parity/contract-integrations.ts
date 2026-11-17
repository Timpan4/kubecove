import { type StepParsers, boolean, count, isBoolean, oneOf, optionalBoolean, plainString, string, strings, noPayload } from "./contract-values";

const ARGO_OPTION_NAMES = ["Prune", "Dry run", "Force / replace"] as const;

export const integrationsSteps = {
	controlClock: noPayload,
	advanceClock: (item) => ({ ms: count(item.ms) }),
	remoteSessionSignal: (item) => ({ state: oneOf(["reconnecting", "sessionEnded", "signInExpired", "sessionExpired"], item.state, "a remote session signal") }),
	expectRemoteNotice: (item) => ({ value: string(item.value) }),
	expectNoRemoteNotice: noPayload,
	clickRemoteNoticeButton: (item) => ({ label: oneOf(["Sign in", "Copy sign-in link", "Cancel", "Start a new session", "Disconnect"], item.label, "a remote notice button") }),
	fillMcpPort: (item) => ({ value: string(item.value) }),
	mcpControl: (item) => ({ control: oneOf(["MCP server", "Submit port", "Save", "Copy token", "Copy Claude Code command", "Rotate token", "Reload settings"], item.control, "an MCP control") }),
	configureMcpSettingsFile: (item) => ({ file: oneOf(["invalid", "valid"], item.file, "an MCP settings file state") }),
	expectMcpLoadError: (item) => ({ text: string(item.text) }),
	expectMcpState: (item) => {
		if (!isBoolean(item.enabled) || !isBoolean(item.running) || !isBoolean(item.save) || !isBoolean(item.command)) throw new Error("Expected MCP control states");
		return { enabled: item.enabled, port: string(item.port), running: item.running, save: item.save, command: item.command };
	},
	expectMcpCopy: (item) => {
		if (!isBoolean(item.rotated)) throw new Error("Expected rotation state");
		return { kind: oneOf(["token", "command"], item.kind, "an MCP copy kind"), rotated: item.rotated };
	},
	expectMcpRequests: (item) => ({ requests: strings(item.requests, true) }),
	selectArgoPreference: (item) => ({ preference: oneOf(["Automatic", "Kubernetes"], item.preference, "an inspection preference") }),
	expectSavedArgoProfile: (item) => ({ url: string(item.url) }),
	configureArgoSync: (item) => ({ revision: string(item.revision), name: string(item.name) }),
	selectArgoHistory: (item) => ({ revision: string(item.revision) }),
	expectArgoWorkspaceTransport: (item) => ({ text: string(item.text) }),
	confirmArgoTunnel: (item) => ({ failure: item.failure == null ? undefined : string(item.failure) }),
	completeArgoOperation: noPayload,
	connectArgoProfile: (item) => ({ url: string(item.url), remember: optionalBoolean(item.remember), tunnel: optionalBoolean(item.tunnel), local: optionalBoolean(item.local), insecure: optionalBoolean(item.insecure), customCa: item.customCa == null ? undefined : string(item.customCa), surface: item.surface == null ? undefined : oneOf(["page"] as const, item.surface, "an Argo connection surface") }),
	disconnectArgoProfile: noPayload,
	confirmArgoReview: noPayload,
	cancelArgoReview: noPayload,
	configureArgoMock: (item) => ({ script: oneOf(["base", "discovery-error", "target-mismatch", "preview-error", "snapshot-error", "operation-error", "lazy", "full-diff", "connect-pending", "disconnect-pending", "credential-error", "preview-pending"], item.script, "an Argo fixture script") }),
	argoControl: (item) => ({ control: oneOf(["Open connection", "Close connection", "Disconnect", "Forget", "Cancel sign-in", "Retry", "Confirm target again", "Retry operation", "Retry state refresh", "Retry snapshot", "Technical detail", "Show diff", "Show fewer lines", "All changes", "Sync options", "Sync with these options", "Refresh options"], item.control, "an Argo control"), url: item.url == null ? undefined : string(item.url) }),
	focusArgoResource: (item) => ({ key: string(item.key), label: string(item.label) }),
	/** Seeds a legacy profile ID before restart. */
	ageArgoProfileId: (item) => ({ url: string(item.url) }),
	expectNoSavedArgoProfile: (item) => ({ url: string(item.url) }),
	expandArgoDiff: noPayload,
	reconnectArgoProfile: noPayload,
	argoProfileSignIn: (item) => ({ token: plainString(item.token), tunnel: optionalBoolean(item.tunnel), pending: optionalBoolean(item.pending), submit: optionalBoolean(item.submit) }),
	configureArgoInventory: (item) => ({ mode: oneOf(["base", "absent", "empty", "partial", "unavailable", "many", "same-name"], item.mode, "an Argo inventory mode") }),
	copyArgoValue: (item) => ({ control: oneOf(["source", "revision", "repository value", "release value", "cluster target value"], item.control, "an Argo copy control"), contains: string(item.contains) }),
	reviewArgoOperation: (item) => ({ operation: oneOf(["Refresh", "Hard refresh", "Sync", "Rollback"], item.operation, "an Argo operation") }),
	expectArgoReview: (item) => ({ operation: string(item.operation), name: string(item.name), namespace: string(item.namespace), transport: string(item.transport) }),
	expectArgoText: (item) => ({ text: string(item.text) }),
	expectArgoPolicy: (item) => ({ text: string(item.text) }),
	expectArgoPopover: (item) => ({ control: oneOf(["Sync options", "Refresh options"], item.control, "an Argo options popover"), open: boolean(item.open) }),
	dismissArgoPopover: (item) => ({ how: oneOf(["Escape", "outside click"], item.how, "a popover dismissal") }),
	expectArgoCheckbox: (item) => ({ name: oneOf(ARGO_OPTION_NAMES, item.name, "an Argo option"), checked: boolean(item.checked) }),
	toggleArgoOption: (item) => ({ name: oneOf(ARGO_OPTION_NAMES, item.name, "an Argo option") }),
	expectNoArgoControl: (item) => ({ control: oneOf(["Confirm target again", "Reconnect", "Confirm and connect"], item.control, "an Argo control") }),
	expectArgoFocusInPopover: (item) => ({ control: oneOf(["Sync options", "Refresh options"], item.control, "an Argo options popover") }),
	fillArgoConnectionField: (item) => ({ field: oneOf(["url", "rootPath", "tlsServerName", "token", "password"], item.field, "an Argo connection field"), value: plainString(item.value) }),
	expectArgoConnectionField: (item) => ({ field: oneOf(["url", "rootPath", "tlsServerName", "token", "password"], item.field, "an Argo connection field"), value: plainString(item.value) }),
	argoConnectionDialog: (item) => ({ operation: oneOf(["open", "expectFocusIn", "tabCycle", "shiftTabCycle", "escape", "outsideClick", "expectOpen", "expectConnectForm", "expectRemember", "expectLocalLogin", "expectInsecure", "chooseLocalLogin", "chooseInsecure", "openDetail", "chooseFoundInCluster", "chooseExternalUrl", "expectFoundInCluster", "expectInWindow", "expectReachable"], item.operation, "an Argo connection dialog operation"), open: optionalBoolean(item.open) === true, checked: optionalBoolean(item.checked) === true, control: item.operation === "expectReachable" ? oneOf(["External URL", "Token", "Remember credential", "Connect"], item.control, "an Argo connection dialog control") : "" }),
	tabArgoPopover: (item) => ({ control: oneOf(["Sync options", "Refresh options"], item.control, "an Argo options popover"), presses: count(item.presses) }),
	expectArgoTriggerFocus: (item) => ({ control: oneOf(["Sync options", "Refresh options"], item.control, "an Argo options trigger") }),
	expectArgoOptionFocusable: (item) => ({ name: oneOf(["Prune", "Dry run", "Force / replace"], item.name, "an Argo sync option") }),
	expectArgoOperationState: (item) => ({ control: oneOf(["Sync", "Refresh", "Sync options", "Refresh options"], item.control, "an Argo operation control"), enabled: boolean(item.enabled) }),
	selectArgoConnection: (item) => ({ url: string(item.url) }),
	argoDocument: (item) => ({ view: oneOf(["Desired", "Live", "Changes"], item.view, "an Argo document"), contains: string(item.contains) }),
} satisfies StepParsers;
