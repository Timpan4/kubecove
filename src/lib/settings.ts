import { persist } from "zustand/middleware";
import { createStore } from "zustand/vanilla";
import {
	type ArgoConnectionPreference,
	normalizeArgoConnectionPreference,
} from "./argo-connection-policy";
import type { ArgoServerEndpoint } from "./gitops-types";
import type {
	KubeconfigSourcesSummary,
	YamlEncoding,
	YamlViewMode,
} from "./types";

export type TimestampTimezone = "local" | "utc";
export type YamlDiffStyle = "clean" | "git";
export type GitOpsViewMode = "cards" | "list";
export type HelmViewMode = "cards" | "list";
export type SavedArgoProfile = {
	id: string;
	endpoint: ArgoServerEndpoint;
	clusterContext?: string;
	workspaceId?: string;
	kubeconfigSourceKey?: string | null;
	rememberCredential: boolean;
};
export const DEFAULT_KUBECONFIG_ENV_VAR = "KUBECONFIG";
const DEFAULT_KUBECONFIG_SOURCE_KEY = "kubeconfigSource=default";

export interface SettingsState {
	showExactTimestamps: boolean;
	showUsageFooter: boolean;
	showFullTopologyOnSelection: boolean;
	showUnavailableGitOpsProviders: boolean;
	redactSecrets: boolean;
	argoProfiles: SavedArgoProfile[];
	argoConnectionPreferences: Record<string, ArgoConnectionPreference>;
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
	setShowFullTopologyOnSelection: (show: boolean) => void;
	setShowUnavailableGitOpsProviders: (show: boolean) => void;
	setRedactSecrets: (redact: boolean) => void;
	setArgoProfiles: (profiles: SavedArgoProfile[]) => void;
	setArgoConnectionPreference: (workspaceId: string, preference: ArgoConnectionPreference) => void;
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
		showFullTopologyOnSelection:
			saved.showFullTopologyOnSelection ?? current.showFullTopologyOnSelection,
		showUnavailableGitOpsProviders:
			saved.showUnavailableGitOpsProviders ?? current.showUnavailableGitOpsProviders,
		redactSecrets: saved.redactSecrets ?? current.redactSecrets,
		argoProfiles: Array.isArray(saved.argoProfiles)
			? saved.argoProfiles.flatMap((profile) => {
					const normalized = normalizeSavedArgoProfile(profile);
					return normalized ? [normalized] : [];
				})
			: current.argoProfiles,
		argoConnectionPreferences: normalizeArgoConnectionPreferences(
			saved.argoConnectionPreferences,
			current.argoConnectionPreferences,
		),
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

function normalizeArgoConnectionPreferences(
	value: unknown,
	fallback: Record<string, ArgoConnectionPreference>,
): Record<string, ArgoConnectionPreference> {
	if (typeof value !== "object" || value === null || Array.isArray(value)) return fallback;
	return Object.fromEntries(
		Object.entries(value).flatMap(([workspaceId, preference]) =>
			workspaceId.trim() ? [[workspaceId, normalizeArgoConnectionPreference(preference)]] : [],
		),
	);
}

export function normalizeSavedArgoProfile(value: unknown): SavedArgoProfile | null {
	if (typeof value !== "object" || value === null) return null;
	const profile = value as Record<string, unknown>;
	const id = typeof profile.id === "string" ? profile.id.trim() : "";
	if (!id) return null;
	const endpoint = normalizeArgoEndpoint(profile.endpoint, profile.url);
	if (!endpoint) return null;
	return {
		id,
		endpoint,
		...(typeof profile.clusterContext === "string" && profile.clusterContext.trim()
			? { clusterContext: profile.clusterContext }
			: {}),
		...(typeof profile.workspaceId === "string" && profile.workspaceId.trim()
			? { workspaceId: profile.workspaceId }
			: {}),
		...(typeof profile.kubeconfigSourceKey === "string" && profile.kubeconfigSourceKey.trim()
			? { kubeconfigSourceKey: profile.kubeconfigSourceKey.trim() }
			: profile.kubeconfigSourceKey === null
				? { kubeconfigSourceKey: null }
				: {}),
		rememberCredential: Boolean(profile.rememberCredential),
	};
}

function normalizeArgoEndpoint(endpoint: unknown, legacyUrl: unknown): ArgoServerEndpoint | null {
	if (typeof endpoint !== "object" || endpoint === null) {
		return externalHttpsEndpoint(legacyUrl);
	}
	const value = endpoint as Record<string, unknown>;
	if (value.kind === "externalHttps") return externalHttpsEndpoint(value.url);
	if (value.kind !== "serviceTunnel") return null;
	const namespace = typeof value.namespace === "string" ? value.namespace.trim() : "";
	const serviceName = typeof value.serviceName === "string" ? value.serviceName.trim() : "";
	const servicePort = value.servicePort;
	if (
		!namespace ||
		!serviceName ||
		!Number.isInteger(servicePort) ||
		typeof servicePort !== "number" ||
		servicePort < 1 ||
		servicePort > 65535 ||
		(value.scheme !== "https" && value.scheme !== "http")
	) {
		return null;
	}
	const rootPath = typeof value.rootPath === "string" ? value.rootPath.trim() : "";
	const tlsServerName = typeof value.tlsServerName === "string" ? value.tlsServerName.trim() : "";
	return {
		kind: "serviceTunnel",
		namespace,
		serviceName,
		servicePort,
		scheme: value.scheme,
		...(rootPath ? { rootPath } : {}),
		...(tlsServerName ? { tlsServerName } : {}),
	};
}

function externalHttpsEndpoint(value: unknown): ArgoServerEndpoint | null {
	if (typeof value !== "string") return null;
	try {
		return new URL(value).protocol === "https:"
			? { kind: "externalHttps", url: value }
			: null;
	} catch {
		return null;
	}
}

export function partializeSettings(state: SettingsState): Partial<SettingsState> {
	return {
		showExactTimestamps: state.showExactTimestamps,
		showUsageFooter: state.showUsageFooter,
		showFullTopologyOnSelection: state.showFullTopologyOnSelection,
		showUnavailableGitOpsProviders: state.showUnavailableGitOpsProviders,
		redactSecrets: state.redactSecrets,
		argoProfiles: state.argoProfiles,
		argoConnectionPreferences: state.argoConnectionPreferences,
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

export const useSettingsState = createStore<SettingsState>()(
	persist(
		(set) => ({
			showExactTimestamps: false,
			showUsageFooter: false,
			showFullTopologyOnSelection: false,
			showUnavailableGitOpsProviders: false,
			redactSecrets: true,
			argoProfiles: [],
			argoConnectionPreferences: {},
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
			setShowFullTopologyOnSelection: (show: boolean) =>
				set({ showFullTopologyOnSelection: show }),
			setShowUnavailableGitOpsProviders: (show: boolean) =>
				set({ showUnavailableGitOpsProviders: show }),
			setRedactSecrets: (redactSecrets: boolean) => set({ redactSecrets }),
			setArgoProfiles: (argoProfiles: SavedArgoProfile[]) => set({ argoProfiles }),
			setArgoConnectionPreference: (workspaceId, preference) =>
				set((state) => ({
					argoConnectionPreferences: {
						...state.argoConnectionPreferences,
						[workspaceId]: normalizeArgoConnectionPreference(preference),
					},
				})),
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
