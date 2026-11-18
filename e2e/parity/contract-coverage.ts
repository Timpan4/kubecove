import { type StepParsers, boolean, oneOf, string, noPayload } from "./contract-values";

export const coverageSteps = {
	coverageSelect: (item) => ({ label: string(item.label), value: string(item.value) }),
	coverageCopy: (item) => ({ label: string(item.label), text: string(item.text), match: oneOf(["contains", "exact"], item.match, "a clipboard match") }),
	coverageScript: (item) => ({ script: oneOf(["base", "loading", "failed", "empty", "partial", "many", "conflict", "logs-empty", "logs-failed", "logs-options", "logs-long", "logs-stream", "startup-failed", "foreign", "detail-loading", "enrichment-failed", "enrichment-loading", "attention-only", "stop-failed", "stop-pending"], item.script, "a coverage script") }),
	coverageCheck: (item) => ({ check: string(item.check) }),
	coverageClick: (item) => ({ label: string(item.label) }),
	coverageFill: (item) => ({ label: string(item.label), value: item.value === "" ? "" : string(item.value) }),
	coverageText: (item) => ({ text: string(item.text), visible: boolean(item.visible) }),
	paletteKey: (item) => ({ key: oneOf(["ArrowDown", "ArrowUp", "Enter", "Escape"], item.key, "a palette key") }),
	expectPaletteOpen: (item) => ({ open: boolean(item.open) }),
	deleteSavedWorkspace: (item) => ({ name: string(item.name) }),
	importDecision: (item) => ({ choice: oneOf(["Add as new", "Replace", "Skip"], item.choice, "an import decision") }),
	cancelImport: noPayload,
	searchEvents: (item) => ({ value: item.value === "" ? "" : string(item.value) }),
} satisfies StepParsers;
