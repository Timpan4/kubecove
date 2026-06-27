import { readPersistedUiRuntimeMode } from "./lib/ui-runtime";

void (async () => {
	const mode = readPersistedUiRuntimeMode();
	if (mode === "svelte") {
		await import("./main-svelte");
		return;
	}
	await import("./main-react");
})();
