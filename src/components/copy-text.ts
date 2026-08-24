export type ClipboardWriter = Pick<Clipboard, "writeText">;

export async function copyText(
	clipboard: ClipboardWriter | undefined,
	value: string,
	label: string,
): Promise<string> {
	if (!clipboard) return `Could not copy ${label}.`;
	try {
		await clipboard.writeText(value);
		return `Copied ${label}.`;
	} catch {
		return `Could not copy ${label}.`;
	}
}
