import { create } from "zustand";
import { persist } from "zustand/middleware";
import type {
	KubeconfigSourcesSummary,
	YamlEncoding,
	YamlViewMode,
} from "./types";

export type TimestampTimezone = "local" | "utc";
export type YamlDiffStyle = "clean" | "git";
export type GitOpsViewMode = "cards" | "list";
export type HelmViewMode = "cards" | "list";
export interface SavedArgoProfile {
	id: string;
	url: string;
	clusterContext?: string;
	workspaceId?: string;
	rememberCredential: boolean;
}
export const DEFAULT_KUBECONFIG_ENV_VAR = "KUBECONFIG";
const DEFAULT_KUBECONFIG_SOURCE_KEY = "kubeconfigSource=default";

export interface SettingsState {
	showExactTimestamps: boolean;
	showUsageFooter: boolean;
	showOwnershipMapByDefault: boolean;
	showFullTopologyOnSelection: boolean;
	showUnavailableGitOpsProviders: boolean;
	redactSecrets: boolean;
	argoProfiles: SavedArgoProfile[];
	gitOpsViewMode: GitOpsViewMode;
	helmViewMode: HelmViewMode;
	showCustomResources: boolean;
	debugModeEnabled: boolean;
	autoStartSavedPortForwards: boolean;
	keepLiveSessionsOnWorkspaceSwitch: boolean;
	allowYamlForceConflicts: boolean;
	timestampTimezone: TimestampTimezone;
	yamlViewModeDefault: YamlViewMode;
	yamlEncodingDefault: YamlEncoding;
	yamlDiffStyle: YamlDiffStyle;
	yamlErrorLensEnabled: boolean;
	kubeconfigEnvVar: string;
	kubeconfigSourceKey: string;
	kubeconfigSourceLabel: string;
	showKubeconfigSourceLabels: boolean;
	setShowExactTimestamps: (show: boolean) => void;
	setShowUsageFooter: (show: boolean) => void;
	setShowOwnershipMapByDefault: (show: boolean) => void;
	setShowFullTopologyOnSelection: (show: boolean) => void;
	setShowUnavailableGitOpsProviders: (show: boolean) => void;
	setRedactSecrets: (redact: boolean) => void;
	setArgoProfiles: (profiles: SavedArgoProfile[]) => void;
	setGitOpsViewMode: (mode: GitOpsViewMode) => void;
	setHelmViewMode: (mode: HelmViewMode) => void;
	setShowCustomResources: (show: boolean) => void;
	setDebugModeEnabled: (enabled: boolean) => void;
	setAutoStartSavedPortForwards: (autoStart: boolean) => void;
	setKeepLiveSessionsOnWorkspaceSwitch: (keep: boolean) => void;
	setAllowYamlForceConflicts: (allow: boolean) => void;
	setTimestampTimezone: (timezone: TimestampTimezone) => void;
	setYamlViewModeDefault: (mode: YamlViewMode) => void;
	setYamlEncodingDefault: (encoding: YamlEncoding) => void;
	setYamlDiffStyle: (style: YamlDiffStyle) => void;
	setYamlErrorLensEnabled: (enabled: boolean) => void;
	setKubeconfigEnvVar: (envVar: string) => void;
	resetKubeconfigEnvVar: () => void;
	setKubeconfigSources: (sources: KubeconfigSourcesSummary) => void;
}

export function normalizeKubeconfigEnvVar(envVar: string | undefined): string {
	const trimmed = envVar?.trim();
	return trimmed || DEFAULT_KUBECONFIG_ENV_VAR;
}

export function kubeconfigSourceKey(envVar: string | undefined): string {
	if (envVar?.startsWith("kubeconfigSource=")) return envVar;
	return `kubeconfigEnv=${normalizeKubeconfigEnvVar(envVar)}`;
}

function isYamlDiffStyle(style: unknown): style is YamlDiffStyle {
	return style === "clean" || style === "git";
}

export function normalizeGitOpsViewMode(mode: unknown): GitOpsViewMode {
	return mode === "list" ? "list" : "cards";
}

export function normalizeHelmViewMode(mode: unknown): HelmViewMode {
	return mode === "list" ? "list" : "cards";
}

export function mergePersistedSettings(persisted: unknown, current: SettingsState): SettingsState {
	const saved =
		typeof persisted === "object" && persisted !== null
			? (persisted as Partial<SettingsState>)
			: {};
	return {
		...current,
		showExactTimestamps: saved.showExactTimestamps ?? current.showExactTimestamps,
		showUsageFooter: saved.showUsageFooter ?? current.showUsageFooter,
		showOwnershipMapByDefault:
			saved.showOwnershipMapByDefault ?? current.showOwnershipMapByDefault,
		showFullTopologyOnSelection:
			saved.showFullTopologyOnSelection ?? current.showFullTopologyOnSelection,
		showUnavailableGitOpsProviders:
			saved.showUnavailableGitOpsProviders ?? current.showUnavailableGitOpsProviders,
		redactSecrets: saved.redactSecrets ?? current.redactSecrets,
		argoProfiles: Array.isArray(saved.argoProfiles)
			? saved.argoProfiles.flatMap((profile) => {
					if (!isSavedArgoProfile(profile)) return [];
					return [{ ...profile, rememberCredential: Boolean(profile.rememberCredential) }];
				})
			: current.argoProfiles,
		gitOpsViewMode: normalizeGitOpsViewMode(saved.gitOpsViewMode),
		helmViewMode: normalizeHelmViewMode(saved.helmViewMode),
		showCustomResources: saved.showCustomResources ?? current.showCustomResources,
		debugModeEnabled: saved.debugModeEnabled ?? current.debugModeEnabled,
		autoStartSavedPortForwards:
			saved.autoStartSavedPortForwards ?? current.autoStartSavedPortForwards,
		keepLiveSessionsOnWorkspaceSwitch:
			saved.keepLiveSessionsOnWorkspaceSwitch ?? current.keepLiveSessionsOnWorkspaceSwitch,
		allowYamlForceConflicts: saved.allowYamlForceConflicts ?? current.allowYamlForceConflicts,
		timestampTimezone: saved.timestampTimezone ?? current.timestampTimezone,
		yamlViewModeDefault: saved.yamlViewModeDefault ?? current.yamlViewModeDefault,
		yamlEncodingDefault: saved.yamlEncodingDefault ?? current.yamlEncodingDefault,
		yamlDiffStyle: isYamlDiffStyle(saved.yamlDiffStyle)
			? saved.yamlDiffStyle
			: current.yamlDiffStyle,
		yamlErrorLensEnabled: saved.yamlErrorLensEnabled ?? current.yamlErrorLensEnabled,
	};
}

function isSavedArgoProfile(value: unknown): value is SavedArgoProfile {
	if (typeof value !== "object" || value === null) return false;
	const profile = value as Partial<SavedArgoProfile>;
	if (typeof profile.id !== "string" || !profile.id.trim()) return false;
	if (typeof profile.url !== "string") return false;
	try {
		return new URL(profile.url).protocol === "https:";
	} catch {
		return false;
	}
}

export function partializeSettings(state: SettingsState): Partial<SettingsState> {
	return {
		showExactTimestamps: state.showExactTimestamps,
		showUsageFooter: state.showUsageFooter,
		showOwnershipMapByDefault: state.showOwnershipMapByDefault,
		showFullTopologyOnSelection: state.showFullTopologyOnSelection,
		showUnavailableGitOpsProviders: state.showUnavailableGitOpsProviders,
		redactSecrets: state.redactSecrets,
		argoProfiles: state.argoProfiles,
		gitOpsViewMode: state.gitOpsViewMode,
		helmViewMode: state.helmViewMode,
		showCustomResources: state.showCustomResources,
		debugModeEnabled: state.debugModeEnabled,
		autoStartSavedPortForwards: state.autoStartSavedPortForwards,
		keepLiveSessionsOnWorkspaceSwitch: state.keepLiveSessionsOnWorkspaceSwitch,
		allowYamlForceConflicts: state.allowYamlForceConflicts,
		timestampTimezone: state.timestampTimezone,
		yamlViewModeDefault: state.yamlViewModeDefault,
		yamlEncodingDefault: state.yamlEncodingDefault,
		yamlDiffStyle: state.yamlDiffStyle,
		yamlErrorLensEnabled: state.yamlErrorLensEnabled,
	};
}

export const useSettingsState = create<SettingsState>()(
	persist(
		(set) => ({
			showExactTimestamps: false,
			showUsageFooter: false,
			showOwnershipMapByDefault: true,
			showFullTopologyOnSelection: false,
			showUnavailableGitOpsProviders: false,
			redactSecrets: true,
			argoProfiles: [],
			gitOpsViewMode: "cards",
			helmViewMode: "cards",
			showCustomResources: true,
			debugModeEnabled: false,
			autoStartSavedPortForwards: false,
			keepLiveSessionsOnWorkspaceSwitch: false,
			allowYamlForceConflicts: true,
			timestampTimezone: "local",
			yamlViewModeDefault: "kubectl",
			yamlEncodingDefault: "yaml",
			yamlDiffStyle: "clean",
			yamlErrorLensEnabled: true,
			kubeconfigEnvVar: DEFAULT_KUBECONFIG_ENV_VAR,
			kubeconfigSourceKey: DEFAULT_KUBECONFIG_SOURCE_KEY,
			kubeconfigSourceLabel: DEFAULT_KUBECONFIG_ENV_VAR,
			showKubeconfigSourceLabels: true,
			setShowExactTimestamps: (show: boolean) =>
				set({ showExactTimestamps: show }),
			setShowUsageFooter: (show: boolean) => set({ showUsageFooter: show }),
			setShowOwnershipMapByDefault: (show: boolean) =>
				set({ showOwnershipMapByDefault: show }),
			setShowFullTopologyOnSelection: (show: boolean) =>
				set({ showFullTopologyOnSelection: show }),
			setShowUnavailableGitOpsProviders: (show: boolean) =>
				set({ showUnavailableGitOpsProviders: show }),
			setRedactSecrets: (redactSecrets: boolean) => set({ redactSecrets }),
			setArgoProfiles: (argoProfiles: SavedArgoProfile[]) => set({ argoProfiles }),
			setGitOpsViewMode: (gitOpsViewMode: GitOpsViewMode) => set({ gitOpsViewMode }),
			setHelmViewMode: (helmViewMode: HelmViewMode) => set({ helmViewMode }),
			setShowCustomResources: (show: boolean) => set({ showCustomResources: show }),
			setDebugModeEnabled: (debugModeEnabled: boolean) =>
				set({ debugModeEnabled }),
			setAutoStartSavedPortForwards: (autoStart: boolean) =>
				set({ autoStartSavedPortForwards: autoStart }),
			setKeepLiveSessionsOnWorkspaceSwitch: (keep: boolean) =>
				set({ keepLiveSessionsOnWorkspaceSwitch: keep }),
			setAllowYamlForceConflicts: (allow: boolean) =>
				set({ allowYamlForceConflicts: allow }),
			setTimestampTimezone: (timezone: TimestampTimezone) =>
				set({ timestampTimezone: timezone }),
			setYamlViewModeDefault: (mode: YamlViewMode) =>
				set({ yamlViewModeDefault: mode }),
			setYamlEncodingDefault: (encoding: YamlEncoding) =>
				set({ yamlEncodingDefault: encoding }),
			setYamlDiffStyle: (yamlDiffStyle: YamlDiffStyle) =>
				set({ yamlDiffStyle }),
			setYamlErrorLensEnabled: (yamlErrorLensEnabled: boolean) =>
				set({ yamlErrorLensEnabled }),
			setKubeconfigEnvVar: (envVar: string) =>
				set({ kubeconfigEnvVar: envVar.trim() }),
			resetKubeconfigEnvVar: () =>
				set({ kubeconfigEnvVar: DEFAULT_KUBECONFIG_ENV_VAR }),
			setKubeconfigSources: (sources: KubeconfigSourcesSummary) =>
				set({
					kubeconfigEnvVar: sources.kubeconfigEnvVar,
					kubeconfigSourceKey: sources.sourceKey,
					kubeconfigSourceLabel: sources.sourceLabel,
					showKubeconfigSourceLabels: sources.showSourceLabels,
				}),
		}),
		{
			name: "kubecove-settings",
			merge: mergePersistedSettings,
			partialize: partializeSettings,
		},
	),
);
