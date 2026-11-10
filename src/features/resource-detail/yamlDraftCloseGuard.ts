import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriRuntime } from "@/lib/tauri-runtime";
import { canLeaveYamlDraft, UNSAVED_YAML_DRAFT_NOTICE } from "./yamlDraftGuard.svelte";

/** Refuses closing the page or window while the open YAML draft has unsaved changes. Returns the cleanup. */
export function installYamlDraftCloseGuard(): () => void {
	return isTauriRuntime() ? guardDesktopWindowClose() : guardBrowserUnload();
}

function guardBrowserUnload(): () => void {
	const refuseUnload = (event: BeforeUnloadEvent) => {
		if (canLeaveYamlDraft()) return;
		event.preventDefault();
		event.returnValue = UNSAVED_YAML_DRAFT_NOTICE;
	};
	window.addEventListener("beforeunload", refuseUnload);
	return () => window.removeEventListener("beforeunload", refuseUnload);
}

// Tauri destroys the window after the handler returns unless the handler called preventDefault.
// The capability grants core:window:allow-destroy for the main window (ADR 0032).
function guardDesktopWindowClose(): () => void {
	let disposed = false;
	let unlisten: (() => void) | undefined;
	void getCurrentWindow()
		.onCloseRequested((event) => {
			if (!canLeaveYamlDraft()) event.preventDefault();
		})
		.then((stop) => {
			if (disposed) stop();
			else unlisten = stop;
		});
	return () => {
		disposed = true;
		unlisten?.();
	};
}
