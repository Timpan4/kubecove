import { useCallback, useMemo, useRef, useState } from "react";
import type { Diagnostic } from "@codemirror/lint";
import { structuredPatch } from "diff";
import { ChevronDown, Pencil, WandSparkles } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import {
	formatYamlDocument,
	YamlCodeViewer,
} from "@/components/YamlCodeViewer";
import {
	YamlEncodingControl,
	YamlViewModeControl,
} from "@/components/YamlModeControl";
import {
	applyYaml,
	getResourceYaml,
	lintKubernetesYaml,
	prepareYamlApply,
	type TauriClient,
} from "../../lib/tauri";
import { useSettingsState } from "../../lib/settings";
import type {
	ResourceSummary,
	YamlEncoding,
	YamlApplyPreview,
	YamlApplyRequest,
	YamlViewMode,
} from "../../lib/types";
import { getErrorMessage } from "./helpers";

const DIFF_CONTEXT_LINES = 3;
const DIFF_PREVIEW_LINE_LIMIT = 24;

interface YamlTabProps {
	client: TauriClient;
	resource: ResourceSummary;
	yaml: string | undefined;
	yamlLoading: boolean;
	yamlError: boolean;
	yamlErr: unknown;
	yamlViewMode: YamlViewMode;
	onYamlViewModeChange: (mode: YamlViewMode) => void;
	yamlEncoding: YamlEncoding;
	onYamlEncodingChange: (encoding: YamlEncoding) => void;
	onApplied: () => void;
}

export function YamlTab({
	client,
	resource,
	yaml,
	yamlLoading,
	yamlError,
	yamlErr,
	yamlViewMode,
	onYamlViewModeChange,
	yamlEncoding,
	onYamlEncodingChange,
	onApplied,
}: YamlTabProps) {
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigEnvVar);
	const [editing, setEditing] = useState(false);
	const [draftYaml, setDraftYaml] = useState("");
	const [draftEncoding, setDraftEncoding] = useState<YamlEncoding>(yamlEncoding);
	const [draftReady, setDraftReady] = useState(false);
	const [preview, setPreview] = useState<YamlApplyPreview | null>(null);
	const [formatError, setFormatError] = useState<unknown>(null);
	const [prepareError, setPrepareError] = useState<unknown>(null);
	const [applyError, setApplyError] = useState<unknown>(null);
	const [appliedMessage, setAppliedMessage] = useState("");
	const [preparing, setPreparing] = useState(false);
	const [applying, setApplying] = useState(false);
	const [loadingDraft, setLoadingDraft] = useState(false);
	const [showFullDiff, setShowFullDiff] = useState(false);
	const secretApplyDisabled =
		resource.kind === "Secret" && (resource.apiVersion ?? "v1") === "v1";
	const activeYamlEncoding = editing ? draftEncoding : yamlEncoding;
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

	const startApplyFlow = async () => {
		setAppliedMessage("");
		setFormatError(null);
		setApplyError(null);
		setPrepareError(null);
		setPreview(null);
		setShowFullDiff(false);
		setEditing(true);
		setDraftEncoding(yamlEncoding);
		setDraftReady(false);
		setLoadingDraft(true);
		try {
			const applyCleanYaml = await getResourceYaml(
				client,
				resource.cluster,
				resource.kind,
				resource.name,
				resource.namespace ?? undefined,
				kubeconfigEnvVar,
				"applyClean",
				yamlEncoding,
			);
			setDraftYaml(applyCleanYaml);
			setDraftReady(true);
		} catch (err) {
			setPrepareError(err);
		} finally {
			setLoadingDraft(false);
		}
	};

	const cancelApplyFlow = () => {
		setEditing(false);
		setDraftYaml("");
		setDraftReady(false);
		setPreview(null);
		setShowFullDiff(false);
		setFormatError(null);
		setPrepareError(null);
		setApplyError(null);
	};

	const buildRequest = (): YamlApplyRequest => ({
		clusterContext: resource.cluster,
		kubeconfigEnvVar,
		kind: resource.kind,
		apiVersion: resource.apiVersion,
		group: resource.group,
		version: resource.version,
		plural: resource.plural,
		namespaced: resource.namespaced,
		name: resource.name,
		namespace: resource.namespace,
		yaml: draftYaml,
		yamlEncoding: activeYamlEncoding,
	});

	const buildRequestForYaml = useCallback(
		(value: string): YamlApplyRequest => ({
			clusterContext: resource.cluster,
			kubeconfigEnvVar,
			kind: resource.kind,
			apiVersion: resource.apiVersion,
			group: resource.group,
			version: resource.version,
			plural: resource.plural,
			namespaced: resource.namespaced,
			name: resource.name,
			namespace: resource.namespace,
			yaml: value,
			yamlEncoding: activeYamlEncoding,
		}),
		[activeYamlEncoding, kubeconfigEnvVar, resource],
	);

	const kubernetesDiagnostics = useCallback(
		async (value: string): Promise<Diagnostic[]> => {
			if (!editing || value.trim().length === 0) return [];
			const result = await lintKubernetesYaml(client, buildRequestForYaml(value));
			return result.diagnostics.map((diagnostic) => {
				const range = findYamlFieldRange(value, diagnostic.fieldPath);
				return {
					from: range.from,
					to: range.to,
					severity: diagnostic.severity,
					source: diagnostic.source,
					message: diagnostic.message,
				};
			});
		},
		[buildRequestForYaml, client, editing],
	);

	const prepare = async () => {
		setPreparing(true);
		setPrepareError(null);
		setApplyError(null);
		setPreview(null);
		setShowFullDiff(false);
		try {
			setPreview(await prepareYamlApply(client, buildRequest()));
		} catch (err) {
			setPrepareError(err);
		} finally {
			setPreparing(false);
		}
	};

	const formatDraft = () => {
		setFormatError(null);
		setPrepareError(null);
		setApplyError(null);
		setPreview(null);
		setShowFullDiff(false);
		try {
			setDraftYaml(formatYamlDocument(draftYaml, activeYamlEncoding));
			setAppliedMessage("");
		} catch (err) {
			setFormatError(err);
		}
	};

	const apply = async () => {
		if (!preview) return;
		setApplying(true);
		setApplyError(null);
		try {
			const result = await applyYaml(client, buildRequest());
			setAppliedMessage(
				`Applied ${result.target.kind}/${result.target.name} with server-side apply.`,
			);
			cancelApplyFlow();
			onApplied();
		} catch (err) {
			setApplyError(err);
		} finally {
			setApplying(false);
		}
	};

	return (
		<div className="flex min-h-0 flex-col">
			<div className="sticky top-0 z-20 flex flex-wrap items-center justify-between gap-2 border-b bg-card px-4 py-3 shadow-sm">
				<div className="flex flex-wrap items-center gap-2">
					<YamlViewModeControl
						value={editing ? "applyClean" : yamlViewMode}
						onChange={onYamlViewModeChange}
						disabled={editing}
					/>
					<YamlEncodingControl
						value={activeYamlEncoding}
						onChange={onYamlEncodingChange}
						disabled={editing}
					/>
				</div>
				<div className="flex items-center gap-2">
					{editing ? (
						<>
							<Button
								type="button"
								variant="outline"
								size="sm"
								disabled={!draftReady || loadingDraft || preparing || applying}
								onClick={formatDraft}
							>
								<WandSparkles data-icon="inline-start" />
								Format
							</Button>
							<Button
								type="button"
								size="sm"
								disabled={!draftReady || loadingDraft || preparing || applying}
								onClick={() => void prepare()}
							>
								{preparing ? "Dry-running..." : "Dry run"}
							</Button>
							<Button
								type="button"
								size="sm"
								disabled={!preview || preparing || applying}
								onClick={() => void apply()}
							>
								{applying ? "Applying..." : "Apply"}
							</Button>
							<Button
								type="button"
								variant="outline"
								size="sm"
								onClick={cancelApplyFlow}
							>
								Cancel
							</Button>
						</>
					) : (
						<Button
							type="button"
							variant="outline"
							size="sm"
							disabled={secretApplyDisabled || yamlLoading || yamlError}
							onClick={() => void startApplyFlow()}
						>
							<Pencil data-icon="inline-start" />
							Edit YAML
						</Button>
					)}
				</div>
			</div>

			<div className="flex flex-col gap-3 p-4">
				{secretApplyDisabled && (
					<Alert>
						<AlertTitle>Secret apply disabled</AlertTitle>
						<AlertDescription>
							v1 Secret YAML contains redacted values, so applying it could
							corrupt data.
						</AlertDescription>
					</Alert>
				)}
				{appliedMessage && (
					<Alert>
						<AlertTitle>Apply complete</AlertTitle>
						<AlertDescription>{appliedMessage}</AlertDescription>
					</Alert>
				)}
				{yamlLoading && (
					<div className="p-6 text-center text-xs text-muted-foreground">
						<Spinner className="mx-auto mb-2 size-4" />
						<span>Loading YAML...</span>
					</div>
				)}
				{yamlError && (
					<Alert variant="destructive">
						<AlertTitle>Failed to load YAML</AlertTitle>
						<AlertDescription>{getErrorMessage(yamlErr)}</AlertDescription>
					</Alert>
				)}
				{!yamlLoading && !yamlError && editing && (
					<>
						<div className="rounded-md border bg-muted/20 p-3 text-xs text-muted-foreground">
							<span className="font-medium text-foreground">
								{resource.cluster}
							</span>{" "}
							{resource.namespace ? `${resource.namespace} / ` : ""}
							{resource.kind} / {resource.name}
						</div>
						<YamlCodeViewer
							value={draftYaml}
							editable
							minHeight="420px"
							extraDiagnostics={kubernetesDiagnostics}
							onChange={(value) => {
								setDraftYaml(value);
								setPreview(null);
								setShowFullDiff(false);
								setFormatError(null);
								setPrepareError(null);
								setApplyError(null);
								setAppliedMessage("");
							}}
						/>
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
						{preview && (
							<div ref={diffContainerRef}>
								<DryRunDiff
									lines={visibleDiffLines}
									isExpanded={showFullDiff}
								/>
								{hasHiddenDiffLines && (
									<div className="mt-2 flex items-center justify-between gap-2 text-xs text-muted-foreground">
										<span>
											{showFullDiff
												? `Showing all ${diffLines.length} lines.`
												: `Showing ${Math.min(
													diffLines.length,
													DIFF_PREVIEW_LINE_LIMIT,
												)} of ${diffLines.length} lines.`}
										</span>
										<Button
											type="button"
											variant="outline"
											size="sm"
											onClick={() => setShowFullDiff((prev) => !prev)}
										>
											{showFullDiff ? "Collapse diff" : "Show full diff"}
										</Button>
									</div>
								)}
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
					</>
				)}
				{!yamlLoading && !yamlError && !editing && yaml && (
					<YamlCodeViewer value={yaml} minHeight="520px" />
				)}
			</div>
		</div>
	);
}

interface UnifiedDiffLine {
	type: "header" | "hunk" | "add" | "remove" | "context" | "empty";
	text: string;
}

function buildCompactUnifiedDiff(
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

function diffLineClassName(type: UnifiedDiffLine["type"]): string {
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

function findYamlFieldRange(value: string, fieldPath?: string): { from: number; to: number } {
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
