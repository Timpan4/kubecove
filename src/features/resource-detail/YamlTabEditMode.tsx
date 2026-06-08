import { useMemo, useRef } from "react";
import { ChevronDown, WandSparkles } from "lucide-react";
import type { Diagnostic } from "@codemirror/lint";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { YamlCodeViewer } from "@/components/YamlCodeViewer";
import { buildCompactUnifiedDiff, diffLineClassName, type UnifiedDiffLine } from "./yamlTabDiff";
import { getErrorMessage } from "./helpers";
import type { ResourceSummary, YamlApplyPreview, YamlEncoding } from "../../lib/types";

const DIFF_PREVIEW_LINE_LIMIT = 24;

interface YamlTabEditModeProps {
	resource: ResourceSummary;
	draftYaml: string;
	draftReady: boolean;
	loadingDraft: boolean;
	preview: YamlApplyPreview | null;
	formatError: unknown;
	prepareError: unknown;
	applyError: unknown;
	appliedMessage: string;
	preparing: boolean;
	applying: boolean;
	showFullDiff: boolean;
	activeYamlEncoding: YamlEncoding;
	onChange: (value: string) => void;
	onFormat: () => void;
	onPrepare: () => Promise<void>;
	onApply: () => Promise<void>;
	onCancel: () => void;
	onHideMessage: () => void;
	onToggleFullDiff: () => void;
	extraDiagnostics?: (value: string) => Promise<Diagnostic[]> | Diagnostic[];
}

function DryRunDiff({
	lines,
	isExpanded,
}: {
	lines: UnifiedDiffLine[];
	isExpanded: boolean;
}) {
	return (
		<div className="overflow-hidden rounded-md border">
			<div className="border-b bg-muted/40 px-3 py-2 text-xs font-medium">
				{isExpanded ? "Dry-run diff (full)" : "Dry-run diff (compact)"}
			</div>
			<pre className="max-h-80 overflow-auto bg-background p-0 font-mono text-xs leading-relaxed">
				{lines.map((line, index) => (
					<span
						key={`${index}-${line.text}`}
						className={`block whitespace-pre px-3 ${diffLineClassName(line.type)}`}
					>
						{line.text}
					</span>
				))}
			</pre>
		</div>
	);
}

export function YamlTabEditMode({
	resource,
	draftYaml,
	draftReady,
	loadingDraft,
	preview,
	formatError,
	prepareError,
	applyError,
	appliedMessage,
	preparing,
	applying,
	showFullDiff,
	activeYamlEncoding,
	onChange,
	onFormat,
	onPrepare,
	onApply,
	onCancel,
	onHideMessage,
	onToggleFullDiff,
	extraDiagnostics,
}: YamlTabEditModeProps) {
	const diffContainerRef = useRef<HTMLDivElement | null>(null);
	const diffLines = useMemo(
		() =>
			preview
				? buildCompactUnifiedDiff(preview.currentYaml, preview.dryRunYaml)
				: [],
		[preview],
	);
	const visibleDiffLines = useMemo(
		() =>
			showFullDiff ? diffLines : diffLines.slice(0, DIFF_PREVIEW_LINE_LIMIT),
		[diffLines, showFullDiff],
	);
	const hasHiddenDiffLines = diffLines.length > DIFF_PREVIEW_LINE_LIMIT;
	const scrollToDiff = () => {
		diffContainerRef.current?.scrollIntoView({
			behavior: "smooth",
			block: "start",
		});
	};

	const onChangeHandled = (value: string) => {
		onHideMessage();
		onChange(value);
	};

	return (
		<>
			<div className="rounded-md border bg-muted/20 p-3 text-xs text-muted-foreground">
				<span className="font-medium text-foreground">{resource.cluster}</span>{" "}
				{resource.namespace ? `${resource.namespace} / ` : ""}
				{resource.kind} / {resource.name}
			</div>
			<div className="flex items-center justify-end gap-2">
				<Button
					type="button"
					variant="outline"
					size="sm"
					disabled={!draftReady || loadingDraft || preparing || applying}
					onClick={onFormat}
				>
					<WandSparkles data-icon="inline-start" />
					Format ({activeYamlEncoding.toUpperCase()})
				</Button>
				<Button
					type="button"
					size="sm"
					disabled={!draftReady || loadingDraft || preparing || applying}
					onClick={() => void onPrepare()}
				>
					{preparing ? "Dry-running..." : "Dry run"}
				</Button>
				<Button
					type="button"
					size="sm"
					disabled={!preview || preparing || applying}
					onClick={() => void onApply()}
				>
					{applying ? "Applying..." : "Apply"}
				</Button>
				<Button type="button" variant="outline" size="sm" onClick={onCancel}>
					Cancel
				</Button>
			</div>
			{appliedMessage && (
				<Alert>
					<AlertTitle>Apply complete</AlertTitle>
					<AlertDescription>{appliedMessage}</AlertDescription>
				</Alert>
			)}
			{formatError && (
				<Alert variant="destructive">
					<AlertTitle>Format failed</AlertTitle>
					<AlertDescription>{getErrorMessage(formatError)}</AlertDescription>
				</Alert>
			)}
			{prepareError && (
				<Alert variant="destructive">
					<AlertTitle>Dry run failed</AlertTitle>
					<AlertDescription>{getErrorMessage(prepareError)}</AlertDescription>
				</Alert>
			)}
			{applyError && (
				<Alert variant="destructive">
					<AlertTitle>Apply failed</AlertTitle>
					<AlertDescription>{getErrorMessage(applyError)}</AlertDescription>
				</Alert>
			)}
			<YamlCodeViewer
				value={draftYaml}
				editable
				minHeight="420px"
				extraDiagnostics={extraDiagnostics}
				onChange={onChangeHandled}
			/>
			{preview && (
				<div ref={diffContainerRef}>
					<DryRunDiff lines={visibleDiffLines} isExpanded={showFullDiff} />
					{hasHiddenDiffLines && (
						<div className="mt-2 flex items-center justify-between gap-2 text-xs text-muted-foreground">
							<span>
								{showFullDiff
									? `Showing all ${diffLines.length} lines.`
									: `Showing ${Math.min(diffLines.length, DIFF_PREVIEW_LINE_LIMIT)} of ${diffLines.length} lines.`}
							</span>
							<Button
								type="button"
								variant="outline"
								size="sm"
								onClick={onToggleFullDiff}
							>
								{showFullDiff ? "Collapse diff" : "Show full diff"}
							</Button>
						</div>
					)}
					{preview && (
						<div className="sticky bottom-3 z-30 flex justify-center py-1">
							<Button
								type="button"
								variant="outline"
								size="sm"
								onClick={scrollToDiff}
								className="bg-card/95"
								aria-label="Go to dry-run diff"
							>
								<ChevronDown data-icon="inline-start" />
								Go to diff
							</Button>
						</div>
					)}
				</div>
			)}
		</>
	);
}
