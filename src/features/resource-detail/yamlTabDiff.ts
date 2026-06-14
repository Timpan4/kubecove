import { structuredPatch } from "diff";

export interface UnifiedDiffLine {
	type: "header" | "hunk" | "add" | "remove" | "context" | "empty";
	text: string;
}

const DIFF_CONTEXT_LINES = 3;

export function buildCompactUnifiedDiff(
	currentYaml: string,

dryRunYaml: string,
): UnifiedDiffLine[] {
	const patch = structuredPatch(
		"current",
		"dry-run",
		currentYaml,
		dryRunYaml,
		"",
		"",
		{ context: DIFF_CONTEXT_LINES },
	);
	const lines: UnifiedDiffLine[] = [
		{ type: "header", text: "--- current" },
		{ type: "header", text: "+++ dry-run" },
	];

	for (const hunk of patch.hunks) {
		lines.push({
			type: "hunk",
			text: `@@ -${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines} @@`,
		});
		for (const line of hunk.lines) {
			if (line.startsWith("+")) {
				lines.push({ type: "add", text: line });
			} else if (line.startsWith("-")) {
				lines.push({ type: "remove", text: line });
			} else if (line.startsWith("\\")) {
				lines.push({ type: "empty", text: line });
			} else {
				lines.push({ type: "context", text: line });
			}
		}
	}

	if (patch.hunks.length === 0) {
		lines.push({ type: "empty", text: "No server-side dry-run changes." });
	}

	return lines;
}

export function diffLineClassName(type: UnifiedDiffLine["type"]): string {
	switch (type) {
		case "add":
			return "bg-emerald-500/12 text-emerald-300";
		case "remove":
			return "bg-destructive/12 text-red-300";
		case "hunk":
			return "bg-primary/10 text-primary";
		case "header":
			return "bg-muted/35 font-semibold text-muted-foreground";
		case "empty":
			return "text-muted-foreground";
		case "context":
		default:
			return "text-muted-foreground";
	}
}

export function findYamlFieldRange(
	value: string,
	fieldPath?: string,
): { from: number; to: number } {
	if (!fieldPath) return { from: 0, to: Math.min(value.length, 1) };
	const key = fieldPath.split(".").at(-1);
	if (!key) return { from: 0, to: Math.min(value.length, 1) };

	const lines = value.split("\n");
	let offset = 0;
	const keyPattern = new RegExp(
		`(?:^|\\s)(?:"${escapeRegExp(key)}"|${escapeRegExp(key)})\\s*:`,
	);
	for (const line of lines) {
		const match = keyPattern.exec(line);
		if (match?.index !== undefined) {
			const from = offset + match.index + (match[0].startsWith(" ") ? 1 : 0);
			return { from, to: Math.min(value.length, from + match[0].trim().length) };
		}
		offset += line.length + 1;
	}

	return { from: 0, to: Math.min(value.length, 1) };
}

function escapeRegExp(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
