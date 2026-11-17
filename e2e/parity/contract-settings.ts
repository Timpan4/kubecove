import { type StepParsers, boolean, count, isBoolean, oneOf, string, noPayload } from "./contract-values";

export const settingsSteps = {
	cancelYamlEdit: noPayload,
	revealSecretValue: noPayload,
	hideSecretValue: noPayload,
	refreshLatencyReport: noPayload,
	clearLatencyReport: noPayload,
	expectLatencyEmpty: noPayload,
	openDocumentation: noPayload,
	expectSecretValue: (item) => {
		if (!isBoolean(item.visible)) throw new Error("Expected Secret visibility");
		return { visible: item.visible };
	},
	expectYamlLens: (item) => {
		return {
			line: count(item.line),
			...(item.marked !== undefined ? { marked: string(item.marked) } : {}),
		};
	},
	expectLatencyTiming: (item) => ({ command: string(item.command) }),
	copyLatencyReport: noPayload,
	expectLatencyReportCopy: (item) => ({ contains: string(item.contains) }),
	expectViewTiming: (item) => ({ view: oneOf(["resources"], item.view, "a timed view") }),
	copyInteractions: noPayload,
	clearInteractions: noPayload,
	expectInteractionCopy: (item) => {
		if (item.contains === undefined && item.excludes === undefined && item.succeededCommand === undefined) {
			throw new Error("Expected contains, excludes or succeededCommand");
		}
		return {
			...(item.contains !== undefined ? { contains: string(item.contains) } : {}),
			...(item.excludes !== undefined ? { excludes: string(item.excludes) } : {}),
			...(item.succeededCommand !== undefined ? { succeededCommand: string(item.succeededCommand) } : {}),
		};
	},
	typeSettingsField: (item) => ({ field: oneOf(["Server token"], item.field, "a typed settings field"), value: string(item.value) }),
	expectUpdateControl: (item) => ({ label: oneOf(["Check for updates", "Update available", "Checking for updates", "Update check failed", "Downloading update", "Restart to finish update"] as const, item.label, "an update control label") }),
	updateStatusControl: (item) => ({ control: oneOf(["Open", "Check", "Later", "Install", "Relaunch", "Escape"], item.control, "an update control") }),
	expectDocumentationRequest: (item) => ({ url: string(item.url) }),
	fillSettingsField: (item) => ({ field: oneOf(["Server URL", "Server token"], item.field, "a remote field"), value: string(item.value) }),
	clickSettingsButton: (item) => ({ label: string(item.label) }),
	clickKubeconfigPathButton: (item) => ({ value: string(item.value), label: string(item.label) }),
	expectKubeconfigPathBefore: (item) => ({ value: string(item.value), other: string(item.other) }),
	addPastedKubeconfigPaths: (item) => ({ value: string(item.value) }),
	fillKubeconfigPathDraft: (item) => ({ value: string(item.value) }),
	expectKubeconfigPathDraft: (item) => ({ value: item.value === "" ? "" : string(item.value), enabled: boolean(item.enabled) }),
	expectAccessibleName: (item) => ({ name: string(item.name) }),
	expectSettingsText: (item) => ({ value: string(item.value) }),
	expectNoSettingsText: (item) => ({ value: string(item.value) }),
	expectRuntimeEndpoint: (item) => ({ value: string(item.value) }),
	expectMaskedToken: noPayload,
	reopenStoredSettings: noPayload,
	configureSettingsMock: (item) => ({ script: oneOf(["oidc-server", "oidc-browser-waits", "oidc-sign-in-fails", "oidc-memory-only", "oidc-store-keeps-sign-in", "access-server", "choose-kubeconfig", "choose-workspace", "choose-unavailable-workspace", "choose-application-workspace", "choose-foreign-kind-workspace", "choose-compare-workspace", "choose-last-export", "cancel-dialog", "no-update", "invalid-signature", "update-available", "update-installs", "update-download-waits"], item.script, "a settings script") }),
	openSettings: noPayload,
	closeSettings: noPayload,
	searchSettings: (item) => ({ value: string(item.value) }),
	setKubeconfigEnvVar: (item) => ({ value: string(item.value) }),
	expectKubeconfigEnvVar: (item) => ({ value: string(item.value) }),
	addServerKubeconfigPath: (item) => ({ value: string(item.value) }),
	expectSettingsEmpty: (item) => ({ value: string(item.value) }),
	showSettingsSection: (item) => ({ section: string(item.section) }),
	expectSettingsHeading: (item) => ({ heading: string(item.heading) }),
	expectSettingRow: (item) => ({ title: string(item.title) }),
	expectNoSettingRow: (item) => ({ title: string(item.title) }),
	selectSettingOption: (item) => ({ setting: string(item.setting), value: string(item.value) }),
	expectSettingOption: (item) => ({ setting: string(item.setting), value: string(item.value) }),
	moveResourceKindEarlier: (item) => ({ kind: string(item.kind) }),
	expectResourceKindPosition: (item) => ({ kind: string(item.kind), lane: string(item.lane), index: count(item.index) }),
	restartClient: noPayload,
	toggleSetting: (item) => ({ setting: string(item.setting) }),
	expectSetting: (item) => {
		if (!isBoolean(item.checked)) throw new Error("Expected a checked state");
		return { setting: string(item.setting), checked: item.checked };
	},
} satisfies StepParsers;
