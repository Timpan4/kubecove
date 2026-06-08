import { Pencil, WandSparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
	YamlEncodingControl,
	YamlViewModeControl,
} from "@/components/YamlModeControl";
import type {
	YamlApplyPreview,
	YamlEncoding,
	YamlViewMode,
} from "../../lib/types";

interface YamlTabHeaderProps {
	editing: boolean;
	yamlLoading: boolean;
	yamlError: boolean;
	secretApplyDisabled: boolean;
	loadingDraft: boolean;
	draftReady: boolean;
	preview: YamlApplyPreview | null;
	preparing: boolean;
	applying: boolean;
	yamlViewMode: YamlViewMode;
	yamlEncoding: YamlEncoding;
	onYamlViewModeChange: (mode: YamlViewMode) => void;
	onYamlEncodingChange: (encoding: YamlEncoding) => void;
	onStartApplyFlow: () => void;
	onFormat: () => void;
	onPrepare: () => Promise<void>;
	onApply: () => Promise<void>;
	onCancel: () => void;
}

export function YamlTabHeader({
	editing,
	yamlLoading,
	yamlError,
	secretApplyDisabled,
	loadingDraft,
	draftReady,
	preview,
	preparing,
	applying,
	yamlViewMode,
	yamlEncoding,
	onYamlViewModeChange,
	onYamlEncodingChange,
	onStartApplyFlow,
	onFormat,
	onPrepare,
	onApply,
	onCancel,
}: YamlTabHeaderProps) {
	return (
		<div className="sticky top-0 z-20 flex flex-wrap items-center justify-between gap-2 border-b bg-card px-4 py-3 shadow-sm">
			<div className="flex flex-wrap items-center gap-2">
				<YamlViewModeControl
					value={editing ? "applyClean" : yamlViewMode}
					onChange={onYamlViewModeChange}
					disabled={editing}
				/>
				<YamlEncodingControl
					value={yamlEncoding}
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
							onClick={onFormat}
						>
							<WandSparkles data-icon="inline-start" />
							Format
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
						<Button
							type="button"
							variant="outline"
							size="sm"
							onClick={onCancel}
						>
							Cancel
						</Button>
					</>
				) : (
					<Button
						type="button"
						variant="outline"
						size="sm"
						disabled={secretApplyDisabled || yamlLoading || yamlError || loadingDraft}
						onClick={onStartApplyFlow}
					>
						<Pencil data-icon="inline-start" />
						Edit YAML
					</Button>
				)}
			</div>
		</div>
	);
}
