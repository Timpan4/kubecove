import type { ChipVariant } from "./constants";

export function incidentSignalCardClassName(tone: ChipVariant): string {
	const base = "rounded-md border border-l-4 p-3";
	switch (tone) {
		case "error":
			return `${base} border-red-500/25 border-l-red-500 bg-red-500/5`;
		case "warning":
			return `${base} border-amber-500/25 border-l-amber-500 bg-amber-500/5`;
		case "info":
			return `${base} border-sky-500/25 border-l-sky-500 bg-sky-500/5`;
		case "success":
			return `${base} border-emerald-500/25 border-l-emerald-500 bg-emerald-500/5`;
		case "neutral":
			return `${base} border-border border-l-muted bg-card`;
	}
}
