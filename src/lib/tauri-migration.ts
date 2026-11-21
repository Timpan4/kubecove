import { SETTINGS_STORAGE_KEY, WORKSPACE_STORAGE_KEY } from "./storage-keys";
import { createNativeTauriClient, isBrowserRuntime, isTauriRuntime } from "./tauri-runtime";

const local = createNativeTauriClient();
let writing = Promise.resolve();

interface MigrationInput {
	settings: string | null;
	workspaces: string | null;
}

export function exportTauriMigrationSnapshot(): Promise<void> {
	if (isBrowserRuntime() || !isTauriRuntime()) return Promise.resolve();
	// Read when this write starts, so an earlier startup export cannot overwrite the handoff.
	writing = writing.catch(() => {}).then(() => {
		const input: MigrationInput = {
			settings: localStorage.getItem(SETTINGS_STORAGE_KEY),
			workspaces: localStorage.getItem(WORKSPACE_STORAGE_KEY),
		};
		return local.invoke<void>("export_tauri_migration_snapshot", { input });
	});
	return writing;
}

export function startTauriMigrationExport(): void {
	if (isBrowserRuntime() || !isTauriRuntime()) return;
	const save = () => {
		void exportTauriMigrationSnapshot().catch(() => console.warn("Could not save the migration snapshot."));
	};
	save();
	document.addEventListener("visibilitychange", () => {
		if (document.hidden) save();
	});
}
