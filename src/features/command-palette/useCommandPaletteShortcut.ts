import { useEffect } from "react";
import { useCommandPaletteStore } from "./store";
import { shouldToggleCommandPaletteShortcut } from "./shortcut";

/**
 * Global ⌘K (macOS) / Ctrl+K (Windows, Linux) listener. Capture phase so it
 * wins over focused inputs; skipped inside the xterm terminal, which owns its
 * own keymap.
 */
export function useCommandPaletteShortcut(): void {
	useEffect(() => {
		const handler = (event: KeyboardEvent) => {
			if (!shouldToggleCommandPaletteShortcut(event)) return;
			event.preventDefault();
			event.stopPropagation();
			useCommandPaletteStore.getState().toggle();
		};
		window.addEventListener("keydown", handler, { capture: true });
		return () => window.removeEventListener("keydown", handler, { capture: true });
	}, []);
}
