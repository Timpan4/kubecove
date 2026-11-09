import { ARGO_OPTION_NAMES, type Step } from "./contract-steps";
import { boolean, count, oneOf, optionalBoolean, string, strings } from "./contract-values";

/** Parses remote connection, MCP server and Argo CD steps; returns undefined for any other action. */
export function integrationStep(item: Record<string, unknown>): Step | undefined {
	switch (item.action) {
		case "expectRemoteHost": return { action: item.action, url: string(item.url), disconnected: optionalBoolean(item.disconnected) };
		case "expectRemoteHostTooltip": return { action: item.action, value: string(item.value) };
		case "expectNoRemoteHost": case "dropRemoteConnection": return { action: item.action };
		case "remoteSessionSignal": return { action: item.action, state: oneOf(["reconnecting", "sessionEnded", "signInExpired", "sessionExpired"], item.state, "a remote session signal") };
		case "expectRemoteNotice": return { action: item.action, value: string(item.value) };
		case "expectNoRemoteNotice": return { action: item.action };
		case "clickRemoteNoticeButton": return { action: item.action, label: oneOf(["Sign in", "Copy sign-in link", "Cancel", "Start a new session", "Disconnect"], item.label, "a remote notice button") };
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
		case "configureArgoMock": return { action: item.action, script: oneOf(["base", "discovery-error", "target-mismatch", "preview-error", "snapshot-error", "operation-error", "lazy", "full-diff", "connect-pending", "disconnect-pending"], item.script, "an Argo fixture script") };
		case "argoControl": return { action: item.action, control: oneOf(["Open connection", "Close connection", "Disconnect", "Forget", "Retry", "Confirm target again", "Retry operation", "Retry state refresh", "Retry snapshot", "Technical detail", "Show diff", "Show fewer lines", "All changes", "Sync options", "Sync with these options", "Refresh options"], item.control, "an Argo control"), url: item.url == null ? undefined : string(item.url) };
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
		case "expectNoArgoControl": return { action: item.action, control: oneOf(["Confirm target again", "Reconnect"], item.control, "an Argo control") };
		case "expectArgoFocusInPopover": return { action: item.action, control: oneOf(["Sync options", "Refresh options"], item.control, "an Argo options popover") };
		case "argoConnectionDialog": return { action: item.action, operation: oneOf(["open", "expectFocusIn", "tab", "shiftTab", "escape", "outsideClick", "expectOpen", "expectConnectForm", "expectRemember", "expectLocalLogin", "expectInsecure", "chooseLocalLogin", "chooseInsecure", "openDetail", "chooseFoundInCluster", "chooseExternalUrl", "expectFoundInCluster"], item.operation, "an Argo connection dialog operation"), presses: item.presses === undefined ? 0 : count(item.presses), open: optionalBoolean(item.open) === true, checked: optionalBoolean(item.checked) === true };
		case "tabArgoPopover": return { action: item.action, control: oneOf(["Sync options", "Refresh options"], item.control, "an Argo options popover"), presses: count(item.presses) };
		case "expectArgoTriggerFocus": return { action: item.action, control: oneOf(["Sync options", "Refresh options"], item.control, "an Argo options trigger") };
		case "expectArgoOptionFocusable": return { action: item.action, name: oneOf(["Prune", "Dry run", "Force / replace"], item.name, "an Argo sync option") };
		case "expectArgoOperationState": return { action: item.action, control: oneOf(["Sync", "Refresh", "Sync options", "Refresh options"], item.control, "an Argo operation control"), enabled: boolean(item.enabled) };
		case "selectArgoConnection": return { action: item.action, url: string(item.url) };
		case "argoDocument": return { action: item.action, view: oneOf(["Desired", "Live", "Changes"], item.view, "an Argo document"), contains: string(item.contains) };
		default: return undefined;
	}
}
