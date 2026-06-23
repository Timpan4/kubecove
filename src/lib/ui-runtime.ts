import {
	decodePathStateWorkspaceHandoff,
	sanitizePathStateWorkspaceHandoff,
	type PathStateWorkspaceHandoff,
} from "./path-state";

export type UiRuntimeMode = "react" | "svelte";
export type UiRuntimeWorkspaceHandoff = PathStateWorkspaceHandoff;

export const DEFAULT_UI_RUNTIME_MODE: UiRuntimeMode = "react";
export const SETTINGS_STORAGE_KEY = "kubecove-settings";
export const UI_RUNTIME_RELOAD_NOTICE_KEY = "kubecove-ui-runtime-reload";
export const UI_RUNTIME_SETTINGS_OPEN_KEY = "kubecove-settings-open";
export const UI_RUNTIME_SETTINGS_FOCUS_KEY = "kubecove-settings-focus";
export const UI_RUNTIME_WORKSPACE_HANDOFF_KEY =
	"kubecove-ui-runtime-workspace-handoff";
export const UI_RUNTIME_SETTINGS_OPEN_VALUE = "ui-runtime";
export const UI_RUNTIME_SETTINGS_FOCUS_VALUE = "ui-runtime";

export function isUiRuntimeMode(value: unknown): value is UiRuntimeMode {
	return value === "react" || value === "svelte";
}

export function uiRuntimeModeLabel(mode: UiRuntimeMode): string {
	return mode === "svelte" ? "Svelte" : "React";
}

function record(value: unknown): Record<string, unknown> | null {
	return typeof value === "object" && value !== null
		? (value as Record<string, unknown>)
		: null;
}

function storage(kind: "local" | "session"): Storage | null {
	if (typeof window === "undefined") return null;
	try {
		return kind === "local" ? window.localStorage : window.sessionStorage;
	} catch {
		return null;
	}
}

export function readPersistedUiRuntimeMode(): UiRuntimeMode {
	const store = storage("local");
	if (!store) return DEFAULT_UI_RUNTIME_MODE;
	try {
		const parsed = record(JSON.parse(store.getItem(SETTINGS_STORAGE_KEY) ?? "null"));
		const state = record(parsed?.state);
		const mode = state?.uiRuntimeMode;
		return isUiRuntimeMode(mode) ? mode : DEFAULT_UI_RUNTIME_MODE;
	} catch {
		return DEFAULT_UI_RUNTIME_MODE;
	}
}

export function writePersistedUiRuntimeMode(mode: UiRuntimeMode): void {
	const store = storage("local");
	if (!store) return;
	try {
		const parsed =
			record(JSON.parse(store.getItem(SETTINGS_STORAGE_KEY) ?? "null")) ?? {};
		const state = record(parsed.state) ?? {};
		const next: Record<string, unknown> = {
			...parsed,
			state: {
				...state,
				uiRuntimeMode: mode,
			},
		};
		if (!("version" in next)) {
			next.version = 0;
		}
		store.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(next));
	} catch {
		store.setItem(
			SETTINGS_STORAGE_KEY,
			JSON.stringify({ state: { uiRuntimeMode: mode }, version: 0 }),
		);
	}
}

export function queueUiRuntimeSettingsFocus(): void {
	const store = storage("session");
	if (!store) return;
	store.setItem(UI_RUNTIME_SETTINGS_FOCUS_KEY, UI_RUNTIME_SETTINGS_FOCUS_VALUE);
}

export function queueUiRuntimeSettingsOpen(): void {
	const store = storage("session");
	if (!store) return;
	store.setItem(UI_RUNTIME_SETTINGS_OPEN_KEY, UI_RUNTIME_SETTINGS_OPEN_VALUE);
}

export function takeUiRuntimeSettingsOpen(): boolean {
	const store = storage("session");
	const queued =
		store?.getItem(UI_RUNTIME_SETTINGS_OPEN_KEY) ===
		UI_RUNTIME_SETTINGS_OPEN_VALUE;
	if (queued) {
		store?.removeItem(UI_RUNTIME_SETTINGS_OPEN_KEY);
	}
	return queued;
}

export function takeUiRuntimeSettingsFocus(): boolean {
	const store = storage("session");
	const queued =
		store?.getItem(UI_RUNTIME_SETTINGS_FOCUS_KEY) ===
		UI_RUNTIME_SETTINGS_FOCUS_VALUE;
	if (queued) {
		store?.removeItem(UI_RUNTIME_SETTINGS_FOCUS_KEY);
	}
	return queued;
}

export function queueUiRuntimeWorkspaceHandoff(
	handoff: string | UiRuntimeWorkspaceHandoff,
): void {
	const store = storage("session");
	const payload =
		sanitizePathStateWorkspaceHandoff(handoff);
	if (!store || !payload) return;
	store.setItem(UI_RUNTIME_WORKSPACE_HANDOFF_KEY, JSON.stringify(payload));
}

export function takeUiRuntimeWorkspaceHandoff(): UiRuntimeWorkspaceHandoff | null {
	const store = storage("session");
	const raw = store?.getItem(UI_RUNTIME_WORKSPACE_HANDOFF_KEY);
	if (raw) {
		store?.removeItem(UI_RUNTIME_WORKSPACE_HANDOFF_KEY);
	}
	return decodePathStateWorkspaceHandoff(raw);
}

export function queueUiRuntimeReloadNotice(mode: UiRuntimeMode): void {
	const store = storage("session");
	if (!store) return;
	store.setItem(UI_RUNTIME_RELOAD_NOTICE_KEY, JSON.stringify({ mode }));
}

export function takeUiRuntimeReloadNotice(): string | null {
	const store = storage("session");
	const raw = store?.getItem(UI_RUNTIME_RELOAD_NOTICE_KEY);
	if (!raw) return null;
	store?.removeItem(UI_RUNTIME_RELOAD_NOTICE_KEY);
	try {
		const parsed = record(JSON.parse(raw));
		const mode = parsed?.mode;
		return isUiRuntimeMode(mode) ? `${uiRuntimeModeLabel(mode)} UI active` : null;
	} catch {
		return null;
	}
}
