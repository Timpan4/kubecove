import { get, readable, type Readable } from "svelte/store";
import { useSettingsState, type SettingsState } from "./settings";

export type SettingsStore = Readable<SettingsState>;

export function createSettingsStore(): SettingsStore {
	return readable(useSettingsState.getState(), (set) => {
		set(useSettingsState.getState());
		return useSettingsState.subscribe(set);
	});
}

export const settingsStore = createSettingsStore();

export function getSettingsSnapshot(): SettingsState {
	return get(settingsStore);
}
