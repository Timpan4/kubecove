import { canLeaveYamlDraft, UNSAVED_YAML_DRAFT_NOTICE } from "./yamlDraftGuard.svelte";

/** Asks the browser to confirm leaving while the open YAML draft has unsaved changes. */
export function installYamlDraftCloseGuard(): () => void {
	const refuseUnload = (event: BeforeUnloadEvent) => {
		if (canLeaveYamlDraft()) return;
		event.preventDefault();
		event.returnValue = UNSAVED_YAML_DRAFT_NOTICE;
	};
	window.addEventListener("beforeunload", refuseUnload);
	return () => window.removeEventListener("beforeunload", refuseUnload);
}
