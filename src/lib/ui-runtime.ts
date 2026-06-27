export const UI_RUNTIME_SETTINGS_OPEN_KEY = "kubecove-settings-open";
export const UI_RUNTIME_SETTINGS_FOCUS_KEY = "kubecove-settings-focus";
export const UI_RUNTIME_SETTINGS_OPEN_VALUE = "settings";
export const UI_RUNTIME_SETTINGS_FOCUS_VALUE = "settings";

function storage(kind: "local" | "session"): Storage | null {
	if (typeof window === "undefined") return null;
	try {
		return kind === "local" ? window.localStorage : window.sessionStorage;
	} catch {
		return null;
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
