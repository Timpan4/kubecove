interface CommandPaletteShortcutEvent {
	altKey: boolean;
	ctrlKey: boolean;
	key: string;
	metaKey: boolean;
	shiftKey: boolean;
	target: EventTarget | null;
}

function hasClosest(
	target: EventTarget | null,
): target is EventTarget & { closest: (selector: string) => Element | null } {
	return (
		typeof target === "object" &&
		target !== null &&
		"closest" in target &&
		typeof (target as { closest?: unknown }).closest === "function"
	);
}

export function shouldToggleCommandPaletteShortcut(
	event: CommandPaletteShortcutEvent,
): boolean {
	if (
		!(event.metaKey || event.ctrlKey) ||
		event.altKey ||
		event.shiftKey ||
		event.key.toLowerCase() !== "k"
	) {
		return false;
	}
	return !hasClosest(event.target) || event.target.closest(".xterm") === null;
}
