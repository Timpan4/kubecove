import { useEffect } from "react";
import { useCommandPaletteStore } from "./store";

/**
 * Global ⌘K (macOS) / Ctrl+K (Windows, Linux) listener. Capture phase so it
 * wins over focused inputs; skipped inside the xterm terminal, which owns its
 * own keymap.
 */
export function useCommandPaletteShortcut(): void {
	useEffect(() => {
		const handler = (event: KeyboardEvent) => {
			if (
				!(event.metaKey || event.ctrlKey) ||
				event.altKey ||
				event.shiftKey ||
				event.key.toLowerCase() !== "k"
			) {
				return;
			}
			if (
				event.target instanceof HTMLElement &&
				event.target.closest(".xterm")
			) {
				return;
			}
			event.preventDefault();
			event.stopPropagation();
			useCommandPaletteStore.getState().toggle();
		};
		window.addEventListener("keydown", handler, { capture: true });
		return () => window.removeEventListener("keydown", handler, { capture: true });
	}, []);
}
