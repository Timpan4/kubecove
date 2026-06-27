import type { CSSProperties } from "react";
import { Badge } from "@/components/ui/badge";
import {
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from "@/components/ui/tooltip";

function isMetadataMap(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hashKeyToHue(key: string): number {
	let hash = 0;
	for (let index = 0; index < key.length; index += 1) {
		hash = (hash * 31 + key.charCodeAt(index)) >>> 0;
	}
	return hash % 360;
}

function stringifyMetadataValue(value: unknown): string {
	if (typeof value === "string") return value;
	if (typeof value === "number" || typeof value === "boolean") {
		return String(value);
	}
	return JSON.stringify(value);
}

function metadataBadgeStyle(key: string): CSSProperties {
	const hue = hashKeyToHue(key);
	return {
		backgroundColor: `hsl(${hue} 70% 22% / 0.38)`,
		borderColor: `hsl(${hue} 72% 52% / 0.46)`,
		color: `hsl(${hue} 88% 78%)`,
	};
}

export function MetadataBadges({ value }: { value: unknown }) {
	if (!isMetadataMap(value)) {
		return <span>{stringifyMetadataValue(value)}</span>;
	}

	const entries = Object.entries(value)
		.map(([key, entryValue]) => [key, stringifyMetadataValue(entryValue)] as const)
		.sort(([leftKey], [rightKey]) => leftKey.localeCompare(rightKey));

	if (entries.length === 0) {
		return <span className="text-muted-foreground">None</span>;
	}

	return (
		<div className="flex w-full min-w-0 flex-col items-start gap-1.5">
			{entries.map(([key, entryValue]) => (
				<Badge
					key={key}
					variant="outline"
					className="h-auto min-h-6 min-w-0 max-w-full justify-start overflow-hidden rounded-sm px-2 py-1 text-left text-xs leading-snug whitespace-nowrap shadow-none"
					style={metadataBadgeStyle(key)}
				>
					<span className="shrink-0 font-semibold">{key}</span>
					<span className="shrink-0 opacity-75">=</span>
					<Tooltip>
						<TooltipTrigger asChild>
							<span
								className="min-w-0 flex-1 truncate"
								tabIndex={0}
								aria-label={`${key} value`}
							>
								{entryValue}
							</span>
						</TooltipTrigger>
						<TooltipContent
							align="start"
							side="top"
							sideOffset={6}
							className="max-w-[min(28rem,calc(100vw-2rem))] whitespace-normal break-words"
						>
							{entryValue}
						</TooltipContent>
					</Tooltip>
				</Badge>
			))}
		</div>
	);
}
