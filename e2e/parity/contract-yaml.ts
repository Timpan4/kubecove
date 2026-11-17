import { type StepParsers, boolean, count, isBoolean, isString, object, oneOf, string, strings, noPayload } from "./contract-values";

const LOG_LEVELS = ["ERR", "WRN", "INF", "DBG"] as const;

/** One diagnostic the YAML editor marks. Positions are 1-based `[line, column]` in UTF-16 columns; `to` is exclusive. Without `message`, the clients' texts may differ. */
export type YamlEditorDiagnostic = { from: [number, number]; to: [number, number]; severity: "error" | "warning" | "info"; source: string; message?: string };

function yamlEditorDiagnostic<Value>(value: Value): YamlEditorDiagnostic {
	const item = object(value);
	const diagnostic: YamlEditorDiagnostic = {
		from: position(item.from),
		to: position(item.to),
		severity: oneOf(["error", "warning", "info"], item.severity, "a diagnostic severity"),
		source: string(item.source),
	};
	if (item.message !== undefined) diagnostic.message = string(item.message);
	return diagnostic;
}

/** A 1-based `[line, column]` editor position. */
function position<Value>(value: Value): [number, number] {
	if (!Array.isArray(value) || value.length !== 2) throw new Error("Expected [line, column]");
	const [line, column] = value.map(count);
	if (!line || !column) throw new Error("Lines and columns start at 1");
	return [line, column];
}

export const yamlSteps = {
	toggleDetailDisclosure: (item) => ({ label: string(item.label) }),
	expectNoPageText: (item) => ({ text: string(item.text) }),
	clickDetailControl: (item) => ({ label: string(item.label) }),
	expectYamlDiagnostic: (item) => {
		if (item.severity !== "error" && item.severity !== "warning" && item.severity !== "info") throw new Error("Expected a diagnostic severity");
		return { severity: item.severity, source: string(item.source), message: string(item.message), path: string(item.path) };
	},
	expectNoYamlDiagnostic: (item) => ({ message: string(item.message) }),
	expectYamlApplyDescription: (item) => ({ text: string(item.text) }),
	expectNoYamlApplyDescription: noPayload,
	expectActionLoadError: (item) => ({ text: string(item.text) }),
	expectNoActionLoadError: noPayload,
	retryActionPermission: noPayload,
	expectActionLock: (item) => {
		if (!isBoolean(item.locked)) throw new Error("Expected a lock state");
		return {
			row: string(item.row), locked: item.locked,
			...(item.reason !== undefined ? { reason: string(item.reason) } : {}),
		};
	},
	expectYamlAbsent: (item) => ({ fragments: strings(item.fragments, false) }),
	clickActionEntry: (item) => ({ label: string(item.label), button: oneOf(["Add", "Remove", "Restore"], item.button, "an entry action") }),
	fillContainerImage: (item) => ({ container: string(item.container), value: string(item.value) }),
	openActionEditor: (item) => ({ row: oneOf(["image", "environment", "resources", "labels", "annotations", "rollback"], item.row, "an action editor") }),
	fillActionField: (item) => ({ label: string(item.label), value: string(item.value) }),
	expectYamlPicker: (item) => ({ label: string(item.label), value: string(item.value) }),
	chooseActionRevision: (item) => ({ revision: count(item.revision) }),
	selectYamlShape: (item) => ({ value: string(item.value) }),
	selectYamlEncoding: (item) => ({ value: string(item.value) }),
	replaceYamlDraft: (item) => ({ value: string(item.value) }),
	expectYamlDocument: (item) => ({ value: string(item.value) }),
	formatYaml: noPayload,
	foldYamlLine: (item) => ({ line: count(item.line) }),
	expectYamlVisibleLines: (item) => {
		if (!Array.isArray(item.lines)) throw new Error("Expected visible line numbers");
		return { lines: item.lines.map(count) };
	},
	showYaml: noPayload,
	expectYaml: (item) => {
		if (!Array.isArray(item.contains) || !item.contains.length) throw new Error("Expected YAML fragments");
		return { contains: item.contains.map(string) };
	},
	showTab: (item) => ({ tab: string(item.tab) }),
	filterEvents: (item) => {
		if (item.filter !== "All" && item.filter !== "Warnings" && item.filter !== "Normal") throw new Error("Expected an event filter");
		return { filter: item.filter };
	},
	expectDetailScroll: (item) => ({ view: oneOf(["events", "timeline"], item.view, "event or timeline view") }),
	expectEvent: (item) => ({ index: count(item.index), reason: string(item.reason), message: string(item.message) }),
	expectEventCount: (item) => ({ count: count(item.count) }),
	closeReview: noPayload,
	expectNoReview: noPayload,
	executeReview: noPayload,
	expectNoActionResult: noPayload,
	editYaml: noPayload,
	applyYaml: noPayload,
	dryRunYaml: noPayload,
	expectYamlReadOnly: noPayload,
	expectLogLines: (item) => {
		// An empty list asserts that no log row renders.
		return { lines: strings(item.lines, true) };
	},
	expectLogToolbarFit: (item) => ({ state: oneOf(["Live", "Paused"], item.state, "a log toolbar state") }),
	filterLogs: (item) => ({ value: item.value === "" ? "" : string(item.value) }),
	editReplicas: (item) => ({ value: string(item.value) }),
	toggleLogLevel: (item) => {
		const level = LOG_LEVELS.find((candidate) => candidate === item.level);
		if (!level) throw new Error("Expected a log level");
		return { level };
	},
	expectLogsEmpty: (item) => ({ title: string(item.title) }),
	expectYamlNotice: (item) => ({ title: string(item.title) }),
	expectNoYamlNotice: (item) => ({ title: string(item.title) }),
	toggleLogRow: (item) => {
		return {
			line: string(item.line),
			...(item.key !== undefined ? { key: oneOf(["Enter", "Space"] as const, item.key, "a log row key") } : {}),
		};
	},
	toggleLogExpandAll: noPayload,
	expectLogRowOpen: (item) => ({ line: string(item.line), open: boolean(item.open) }),
	copyLogLine: (item) => ({ line: string(item.line), contains: string(item.contains) }),
	expectLogTimestampTooltip: (item) => ({ line: string(item.line), value: string(item.value) }),
	expectRestorePrompt: (item) => {
		return {
			visible: boolean(item.visible),
			...(item.count !== undefined ? { count: count(item.count) } : {}),
		};
	},
	restorePromptControl: (item) => ({ control: oneOf(["Start saved", "Review", "Skip", "Auto-start"], item.control, "a restore prompt control") }),
	selectRevision: (item) => ({ label: string(item.label) }),
	reviewAction: (item) => ({ label: string(item.label) }),
	expectRevisionDiff: (item) => ({ selected: string(item.selected), comparison: string(item.comparison), contains: strings(item.contains, false) }),
	expectReview: (item) => ({ title: string(item.title), description: string(item.description), contains: strings(item.contains, false) }),
	showReviewDiff: (item) => {
		return item.key === undefined ? {} : { key: oneOf(["Enter", "Space"] as const, item.key, "a disclosure key") };
	},
	expectDeleteReview: (item) => {
		if (!isString(item.controller)) throw new Error("Expected a controller, empty when the target has none");
		return { name: string(item.name), context: string(item.context), namespace: string(item.namespace), controller: item.controller, consequence: string(item.consequence) };
	},
	expectActionResult: (item) => ({ text: string(item.text) }),
	expectDryRun: (item) => ({ contains: strings(item.contains, false) }),
	expectYamlEditorDiagnostics: (item) => {
		if (!Array.isArray(item.diagnostics)) throw new Error("Expected a list of editor diagnostics");
		return { diagnostics: item.diagnostics.map(yamlEditorDiagnostic) };
	},
} satisfies StepParsers;
