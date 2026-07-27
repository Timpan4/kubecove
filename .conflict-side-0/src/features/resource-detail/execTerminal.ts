import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import type { PodExecTerminalSize } from "@/lib/types";

export interface ExecTerminalHandle {
	readonly cols: number;
	readonly rows: number;
	clear(): void;
	dispose(): void;
	fit(): void;
	write(text: string): void;
	writeln(text: string): void;
}

export function createExecTerminal(
	host: HTMLDivElement,
	onData: (data: string) => void,
	onResize: (size: PodExecTerminalSize) => void,
): ExecTerminalHandle {
	const terminal = new Terminal({
		cursorBlink: true,
		convertEol: true,
		fontFamily: "var(--font-mono, ui-monospace, SFMono-Regular, Consolas, monospace)",
		fontSize: 12,
		theme: {
			background: "#101113",
			foreground: "#f4f4f5",
		},
	});
	const fitAddon = new FitAddon();
	terminal.loadAddon(fitAddon);
	terminal.open(host);
	terminal.onData(onData);
	terminal.onResize(({ cols, rows }) => onResize({ cols, rows }));

	return {
		get cols() {
			return terminal.cols;
		},
		get rows() {
			return terminal.rows;
		},
		clear: () => terminal.clear(),
		dispose: () => terminal.dispose(),
		fit: () => {
			try {
				fitAddon.fit();
			} catch {
				// Terminal can be hidden while tab/panel layout settles.
			}
		},
		write: (text) => terminal.write(text),
		writeln: (text) => terminal.writeln(text),
	};
}
