import { create } from "zustand";
import { persist } from "zustand/middleware";

export type TimestampTimezone = "local" | "utc";
export const DEFAULT_KUBECONFIG_ENV_VAR = "KUBECONFIG";

interface SettingsState {
	showExactTimestamps: boolean;
	showUsageFooter: boolean;
	autoStartSavedPortForwards: boolean;
	timestampTimezone: TimestampTimezone;
	kubeconfigEnvVar: string;
	setShowExactTimestamps: (show: boolean) => void;
	setShowUsageFooter: (show: boolean) => void;
	setAutoStartSavedPortForwards: (autoStart: boolean) => void;
	setTimestampTimezone: (timezone: TimestampTimezone) => void;
	setKubeconfigEnvVar: (envVar: string) => void;
	resetKubeconfigEnvVar: () => void;
}

export function normalizeKubeconfigEnvVar(envVar: string | undefined): string {
	const trimmed = envVar?.trim();
	return trimmed || DEFAULT_KUBECONFIG_ENV_VAR;
}

export function kubeconfigSourceKey(envVar: string | undefined): string {
	return `kubeconfigEnv=${normalizeKubeconfigEnvVar(envVar)}`;
}

export const useSettingsState = create<SettingsState>()(
	persist(
		(set) => ({
			showExactTimestamps: false,
			showUsageFooter: false,
			autoStartSavedPortForwards: false,
			timestampTimezone: "local",
			kubeconfigEnvVar: DEFAULT_KUBECONFIG_ENV_VAR,
			setShowExactTimestamps: (show: boolean) =>
				set({ showExactTimestamps: show }),
			setShowUsageFooter: (show: boolean) => set({ showUsageFooter: show }),
			setAutoStartSavedPortForwards: (autoStart: boolean) =>
				set({ autoStartSavedPortForwards: autoStart }),
			setTimestampTimezone: (timezone: TimestampTimezone) =>
				set({ timestampTimezone: timezone }),
			setKubeconfigEnvVar: (envVar: string) =>
				set({ kubeconfigEnvVar: envVar.trim() }),
			resetKubeconfigEnvVar: () =>
				set({ kubeconfigEnvVar: DEFAULT_KUBECONFIG_ENV_VAR }),
		}),
		{ name: "kubecove-settings" },
	),
);
