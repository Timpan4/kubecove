import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Spinner } from "@/components/ui/spinner";
import { YamlCodeViewer } from "@/components/YamlCodeViewer";
import type { ResourceSummary, YamlApplyPreview, YamlEncoding } from "../../lib/types";
import { getErrorMessage } from "./helpers";
import type { Diagnostic } from "@codemirror/lint";
import { YamlTabEditMode } from "@/features/resource-detail/YamlTabEditMode";

interface YamlTabContentProps {
	secretApplyDisabled: boolean;
	appliedMessage: string;
	yamlLoading: boolean;
	yamlError: boolean;
	yamlErr: unknown;
	editing: boolean;
	yaml: string | undefined;
	resource: ResourceSummary;
	draftYaml: string;
	draftReady: boolean;
	loadingDraft: boolean;
	preview: YamlApplyPreview | null;
	formatError: unknown;
	prepareError: unknown;
	applyError: unknown;
	preparing: boolean;
	applying: boolean;
	showFullDiff: boolean;
	canAllowForceConflicts: boolean;
	activeYamlEncoding: YamlEncoding;
	onEditChange: (value: string) => void;
	onEditFormat: () => void;
	onEditPrepare: () => Promise<void>;
	onEditAllowForceConflicts: () => void;
	onEditApply: () => Promise<void>;
	onEditCancel: () => void;
	onHideEditMessage: () => void;
	onToggleEditFullDiff: () => void;
	extraDiagnostics?: (value: string) => Promise<Diagnostic[]> | Diagnostic[];
}

export function YamlTabContent({
	secretApplyDisabled,
	appliedMessage,
	yamlLoading,
	yamlError,
	yamlErr,
	editing,
	yaml,
	resource,
	draftYaml,
	draftReady,
	loadingDraft,
	preview,
	formatError,
	prepareError,
	applyError,
	preparing,
	applying,
	showFullDiff,
	canAllowForceConflicts,
	activeYamlEncoding,
	onEditChange,
	onEditFormat,
	onEditPrepare,
	onEditAllowForceConflicts,
	onEditApply,
	onEditCancel,
	onToggleEditFullDiff,
	extraDiagnostics,
	onHideEditMessage,
}: YamlTabContentProps) {
	return (
		<div className="flex flex-col gap-3 p-4">
			{secretApplyDisabled && (
				<Alert>
					<AlertTitle>Secret apply disabled</AlertTitle>
					<AlertDescription>
						v1 Secret YAML contains redacted values, so applying it could corrupt
						data.
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
					<span>Loading YAML…</span>
				</div>
			)}
			{yamlError && (
				<Alert variant="destructive">
					<AlertTitle>Failed to load YAML</AlertTitle>
					<AlertDescription>{getErrorMessage(yamlErr)}</AlertDescription>
				</Alert>
			)}
			{!yamlLoading && !yamlError && editing && (
				<YamlTabEditMode
					resource={resource}
					draftYaml={draftYaml}
					draftReady={draftReady}
					loadingDraft={loadingDraft}
					preview={preview}
					formatError={formatError}
					prepareError={prepareError}
					applyError={applyError}
					appliedMessage={appliedMessage}
					preparing={preparing}
					applying={applying}
					showFullDiff={showFullDiff}
					canAllowForceConflicts={canAllowForceConflicts}
					activeYamlEncoding={activeYamlEncoding}
					onChange={onEditChange}
					onFormat={onEditFormat}
					onPrepare={onEditPrepare}
					onAllowForceConflicts={onEditAllowForceConflicts}
					onApply={onEditApply}
					onCancel={onEditCancel}
					onHideMessage={onHideEditMessage}
					onToggleFullDiff={onToggleEditFullDiff}
					extraDiagnostics={extraDiagnostics}
				/>
			)}
			{!yamlLoading && !yamlError && !editing && yaml && (
				<YamlCodeViewer value={yaml} minHeight="520px" />
			)}
		</div>
	);
}
