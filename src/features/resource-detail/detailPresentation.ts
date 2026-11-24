import { CHIP_BADGE_STYLES, type ChipVariant } from "./constants";
import { isCleanCompletedContainer, type ContainerStatusRow } from "./helpers";
import type { IncidentTimelineTone } from "./incident-timeline";

function hashMetadataKeyToHue(key: string): number {
	let hash = 0;
	for (let index = 0; index < key.length; index += 1) {
		hash = (hash * 31 + key.charCodeAt(index)) >>> 0;
	}
	return hash % 360;
}

export function metadataBadgeStyle(key: string): string {
	const hue = hashMetadataKeyToHue(key);
	return [
		`background-color: hsl(${hue} 70% 22% / 0.38)`,
		`border-color: hsl(${hue} 72% 52% / 0.46)`,
		`color: hsl(${hue} 88% 78%)`,
	].join("; ");
}

export function incidentToneClass(tone: IncidentTimelineTone | "success"): string {
	switch (tone) {
		case "error":
			return "border-l-red-500 bg-red-500/5";
		case "warning":
			return "border-l-amber-500 bg-amber-500/5";
		case "info":
			return "border-l-sky-500 bg-sky-500/5";
		case "success":
			return "border-l-emerald-500 bg-emerald-500/5";
		case "neutral":
			return "border-l-muted bg-background/50";
	}
}

export function toneBadgeVariant(tone: ChipVariant): "secondary" | "destructive" | "outline" {
	return CHIP_BADGE_STYLES[tone].variant;
}

function toneBadgeClass(tone: ChipVariant, extra = ""): string {
	return `${CHIP_BADGE_STYLES[tone].className} ${extra}`.trim();
}

export function compactToneBadgeClass(tone: ChipVariant): string {
	return toneBadgeClass(tone, "rounded-full px-2 py-0 text-[0.6875rem] shadow-none");
}

export function restartsTone(restarts: number | undefined): ChipVariant {
	return restarts !== undefined && restarts > 0 ? "warning" : "neutral";
}

export function containerTone(container: ContainerStatusRow): ChipVariant {
	if (isCleanCompletedContainer(container)) return "neutral";
	if (container.exitCode !== undefined && container.exitCode !== 0) return "error";
	if (
		container.state === "waiting" ||
		(container.lastExitCode !== undefined && container.lastExitCode !== 0)
	) {
		return "warning";
	}
	if (container.ready === false) return "error";
	if (container.ready === true || container.state === "running") return "success";
	return "neutral";
}

export function containerReadyLabel(container: ContainerStatusRow): string {
	if (isCleanCompletedContainer(container)) return "Completed";
	if (container.ready === undefined) return "Not reported";
	return container.ready ? "Yes" : "No";
}

export function containerStateLabel(container: ContainerStatusRow): string {
	const state = container.state ?? "-";
	const reason = container.reason ?? container.lastReason;
	return reason && state !== "-" ? `${state} - ${reason}` : (reason ?? state);
}
