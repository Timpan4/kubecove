import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

type Identity = { kind: string; name: string; namespace: string };
export type Step =
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
	| { action: "toggleSetting"; setting: string }
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
	"selectTopologyNode", "expectTopologyEmpty", "expectUsageFooter", "openUsageTree", "expectNoUsageFooter", "expectUsageProcess", "setTopologyMode", "selectNamespace", "expectTopology", "expectTopologyEdge", "expectNoTopologyNode", "expectResourceMetrics",
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
	"expectKubeconfigEnvVar", "toggleSetting", "expectSetting", "openPalette", "searchPalette", "expectPaletteResource", "expectNoPaletteResource",
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
function step(value: unknown): Step {
	const item = object(value);
	switch (item.action) {
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
