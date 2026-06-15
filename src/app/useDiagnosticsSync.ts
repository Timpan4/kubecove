import { useEffect } from "react";
import { diagnosticLog, setDiagnosticsEnabled } from "@/lib/diagnostics";
import { useSettingsState } from "@/lib/settings";
import { createTauriClient, setBackendDiagnosticsEnabled } from "@/lib/tauri";

export function useDiagnosticsSync() {
	const debugModeEnabled = useSettingsState((state) => state.debugModeEnabled);

	useEffect(() => {
		setDiagnosticsEnabled(debugModeEnabled);
		const client = createTauriClient();
		void setBackendDiagnosticsEnabled(client, debugModeEnabled).catch((error) => {
			diagnosticLog("diagnostics.backend.toggle.error", {
				error: error instanceof Error ? error.message : String(error),
			});
		});
	}, [debugModeEnabled]);
}
