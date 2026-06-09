import { create } from "zustand";
import { persist } from "zustand/middleware";
import type {
	KubeconfigSourcesSummary,
	YamlEncoding,
	YamlViewMode,
} from "./types";

export type TimestampTimezone = "local" | "utc";
export const DEFAULT_KUBECONFIG_ENV_VAR = "KUBECONFIG";
const DEFAULT_KUBECONFIG_SOURCE_KEY = "kubeconfigSource=default";

interface SettingsState {
	showExactTimestamps: boolean;
	showUsageFooter: boolean;
	showOwnershipMapByDefault: boolean;
	autoStartSavedPortForwards: boolean;
	keepLiveSessionsOnWorkspaceSwitch: boolean;
	allowYamlForceConflicts: boolean;
	timestampTimezone: TimestampTimezone;
	yamlViewModeDefault: YamlViewMode;
	yamlEncodingDefault: YamlEncoding;
	kubeconfigEnvVar: string;
	kubeconfigSourceKey: string;
	kubeconfigSourceLabel: string;
	showKubeconfigSourceLabels: boolean;
	setShowExactTimestamps: (show: boolean) => void;
	setShowUsageFooter: (show: boolean) => void;
	setShowOwnershipMapByDefault: (show: boolean) => void;
	setAutoStartSavedPortForwards: (autoStart: boolean) => void;
	setKeepLiveSessionsOnWorkspaceSwitch: (keep: boolean) => void;
	setAllowYamlForceConflicts: (allow: boolean) => void;
	setTimestampTimezone: (timezone: TimestampTimezone) => void;
	setYamlViewModeDefault: (mode: YamlViewMode) => void;
	setYamlEncodingDefault: (encoding: YamlEncoding) => void;
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

export const useSettingsState = create<SettingsState>()(
	persist(
		(set) => ({
			showExactTimestamps: false,
			showUsageFooter: false,
			showOwnershipMapByDefault: true,
			autoStartSavedPortForwards: false,
			keepLiveSessionsOnWorkspaceSwitch: false,
			allowYamlForceConflicts: true,
			timestampTimezone: "local",
			yamlViewModeDefault: "kubectl",
			yamlEncodingDefault: "yaml",
			kubeconfigEnvVar: DEFAULT_KUBECONFIG_ENV_VAR,
			kubeconfigSourceKey: DEFAULT_KUBECONFIG_SOURCE_KEY,
			kubeconfigSourceLabel: DEFAULT_KUBECONFIG_ENV_VAR,
			showKubeconfigSourceLabels: true,
			setShowExactTimestamps: (show: boolean) =>
				set({ showExactTimestamps: show }),
			setShowUsageFooter: (show: boolean) => set({ showUsageFooter: show }),
			setShowOwnershipMapByDefault: (show: boolean) =>
				set({ showOwnershipMapByDefault: show }),
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
			merge: (persisted, current) => {
				const saved =
					typeof persisted === "object" && persisted !== null
						? (persisted as Partial<SettingsState>)
						: {};
				return {
					...current,
					showExactTimestamps:
						saved.showExactTimestamps ?? current.showExactTimestamps,
					showUsageFooter: saved.showUsageFooter ?? current.showUsageFooter,
					showOwnershipMapByDefault:
						saved.showOwnershipMapByDefault ??
						current.showOwnershipMapByDefault,
					autoStartSavedPortForwards:
						saved.autoStartSavedPortForwards ??
						current.autoStartSavedPortForwards,
					keepLiveSessionsOnWorkspaceSwitch:
						saved.keepLiveSessionsOnWorkspaceSwitch ??
						current.keepLiveSessionsOnWorkspaceSwitch,
					allowYamlForceConflicts:
						saved.allowYamlForceConflicts ??
						current.allowYamlForceConflicts,
					timestampTimezone:
						saved.timestampTimezone ?? current.timestampTimezone,
					yamlViewModeDefault:
						saved.yamlViewModeDefault ?? current.yamlViewModeDefault,
					yamlEncodingDefault:
						saved.yamlEncodingDefault ?? current.yamlEncodingDefault,
				};
			},
			partialize: (state) => ({
				showExactTimestamps: state.showExactTimestamps,
				showUsageFooter: state.showUsageFooter,
				showOwnershipMapByDefault: state.showOwnershipMapByDefault,
				autoStartSavedPortForwards: state.autoStartSavedPortForwards,
				keepLiveSessionsOnWorkspaceSwitch:
					state.keepLiveSessionsOnWorkspaceSwitch,
				allowYamlForceConflicts: state.allowYamlForceConflicts,
				timestampTimezone: state.timestampTimezone,
				yamlViewModeDefault: state.yamlViewModeDefault,
				yamlEncodingDefault: state.yamlEncodingDefault,
			}),
		},
	),
);
