import { SvelteSet } from "svelte/reactivity";

// The open YAML pane registers whether its draft has unsaved changes, so actions that would
// close or replace the pane refuse first, as the GPUI inspector's `can_leave` does.
export const UNSAVED_YAML_DRAFT_NOTICE =
	"This local draft has unsaved changes. Discard the local draft before changing target or closing.";

const openDrafts = new SvelteSet<() => boolean>();
let refused = $state(false);

/** Returns the unregister function. Unregistering clears the notice, as closing the GPUI detail does. */
export function registerYamlDraft(dirty: () => boolean): () => void {
	openDrafts.add(dirty);
	return () => {
		openDrafts.delete(dirty);
		refused = false;
	};
}

/** True when no open draft has unsaved changes. Otherwise shows the notice and returns false. */
export function canLeaveYamlDraft(): boolean {
	for (const dirty of openDrafts) {
		if (dirty()) {
			refused = true;
			return false;
		}
	}
	return true;
}

export function clearYamlDraftNotice(): void {
	refused = false;
}

export const yamlDraftNotice = {
	get current(): string | null {
		return refused ? UNSAVED_YAML_DRAFT_NOTICE : null;
	},
};
