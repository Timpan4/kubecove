import { useCallback, useEffect, useMemo, useState } from "react";
import { ArrowDown, ArrowUp, FilePlus, Plus, RotateCcw, Trash2 } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { ToggleButton } from "@/components/ToggleButton";
import { DEFAULT_KUBECONFIG_ENV_VAR, useSettingsState } from "@/lib/settings";
import {
	addKubeconfigPaths,
	createTauriClient,
	getKubeconfigSources,
	pickKubeconfigPaths,
	removeKubeconfigPath,
	reorderKubeconfigPaths,
	setKubeconfigEnvVar as setBackendKubeconfigEnvVar,
	setShowKubeconfigSourceLabels,
} from "@/lib/tauri";
import type { KubeconfigSourcesSummary } from "@/lib/types";
import type { SettingsRowMeta } from "./SettingsControls";
import { SettingsBlock, SettingsRow, SettingsSection } from "./SettingsControls";

const ROW_META = {
	activeSources: {
		title: "Active sources",
		description: "Where contexts are discovered from. Full file paths are shown only here.",
	},
	envVar: {
		title: "Environment variable",
		description: "Variable holding kubeconfig paths. The default is read when it is unset.",
	},
	sourceLabels: {
		title: "Show source labels",
		description: "Marks contexts with the kubeconfig source they were discovered from.",
	},
	addedPaths: {
		title: "Added kubeconfig paths",
		description: "Extra kubeconfig files KubeCove reads alongside the environment variable.",
	},
} satisfies Record<string, SettingsRowMeta>;

export const KUBECONFIG_SETTINGS_ROWS: SettingsRowMeta[] = Object.values(ROW_META);

function pathFileName(path: string): string {
	return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

function sourceDescription(sources: KubeconfigSourcesSummary | null): string {
	if (!sources) return "Loading kubeconfig sources.";
	return `${sources.sourceLabel}. Full file paths are shown only here.`;
}

export function KubeconfigSourcesSection({ showTitle = true }: { showTitle?: boolean }) {
	const {
		kubeconfigEnvVar,
		showKubeconfigSourceLabels,
		setKubeconfigEnvVar,
		resetKubeconfigEnvVar,
		setKubeconfigSources,
	} = useSettingsState();
	const client = useMemo(() => createTauriClient(), []);
	const [sources, setSources] = useState<KubeconfigSourcesSummary | null>(null);
	const [envDraft, setEnvDraft] = useState(kubeconfigEnvVar);
	const [pathDraft, setPathDraft] = useState("");
	const [sourceBusy, setSourceBusy] = useState(false);
	const [sourceError, setSourceError] = useState<string | null>(null);

	const applySources = useCallback((next: KubeconfigSourcesSummary) => {
		setSources(next);
		setKubeconfigSources(next);
		setEnvDraft(next.kubeconfigEnvVar);
		setSourceError(null);
	}, [setKubeconfigSources]);

	useEffect(() => {
		void getKubeconfigSources(client)
			.then(applySources)
			.catch((error) =>
				setSourceError(error instanceof Error ? error.message : String(error)),
			);
	}, [applySources, client]);

	const runSourceUpdate = async (
		action: () => Promise<KubeconfigSourcesSummary>,
	) => {
		setSourceBusy(true);
		try {
			applySources(await action());
		} catch (error) {
			setSourceError(error instanceof Error ? error.message : String(error));
		} finally {
			setSourceBusy(false);
		}
	};

	const saveEnvVar = () =>
		void runSourceUpdate(() => setBackendKubeconfigEnvVar(client, envDraft));
	const resetEnvVar = () => {
		resetKubeconfigEnvVar();
		setEnvDraft(DEFAULT_KUBECONFIG_ENV_VAR);
		void runSourceUpdate(() =>
			setBackendKubeconfigEnvVar(client, DEFAULT_KUBECONFIG_ENV_VAR),
		);
	};
	const addPastedPaths = () => {
		if (!pathDraft.trim()) return;
		void runSourceUpdate(() => addKubeconfigPaths(client, [pathDraft])).then(() =>
			setPathDraft(""),
		);
	};

	return (
		<SettingsSection title="Kubeconfig" showTitle={showTitle}>
			<SettingsRow
				title={ROW_META.activeSources.title}
				description={sourceDescription(sources)}
			>
				<Badge variant="outline">
					{sources?.sourceLabel ?? DEFAULT_KUBECONFIG_ENV_VAR}
				</Badge>
			</SettingsRow>

			{(sourceError !== null || (sources?.warnings.length ?? 0) > 0) && (
				<div className="flex flex-col gap-2 px-4 py-3">
					{sourceError && (
						<Alert variant="destructive">
							<AlertTitle>Kubeconfig sources failed</AlertTitle>
							<AlertDescription>{sourceError}</AlertDescription>
						</Alert>
					)}
					{sources?.warnings.map((warning) => (
						<Alert key={`${warning.source}:${warning.path ?? warning.message}`}>
							<AlertTitle>Kubeconfig source warning</AlertTitle>
							<AlertDescription>
								{warning.path ? `${warning.path}: ` : ""}
								{warning.message}
							</AlertDescription>
						</Alert>
					))}
				</div>
			)}

			<SettingsRow {...ROW_META.envVar}>
				<div className="flex items-center gap-2">
					<Input
						className="h-8 w-44"
						value={envDraft}
						placeholder={DEFAULT_KUBECONFIG_ENV_VAR}
						onChange={(event) => {
							setEnvDraft(event.target.value);
							setKubeconfigEnvVar(event.target.value);
						}}
						aria-label="Kubeconfig env var"
					/>
					<Button
						type="button"
						variant="outline"
						size="sm"
						disabled={sourceBusy}
						onClick={saveEnvVar}
					>
						Save
					</Button>
					<Button
						type="button"
						variant="ghost"
						size="icon-sm"
						disabled={sourceBusy}
						aria-label="Reset kubeconfig env var"
						onClick={resetEnvVar}
					>
						<RotateCcw />
					</Button>
				</div>
			</SettingsRow>

			<SettingsRow {...ROW_META.sourceLabels}>
				<ToggleButton
					checked={showKubeconfigSourceLabels}
					onCheckedChange={(show) =>
						void runSourceUpdate(() =>
							setShowKubeconfigSourceLabels(client, show),
						)
					}
					ariaLabel="Show kubeconfig source labels"
				/>
			</SettingsRow>

			<SettingsBlock
				{...ROW_META.addedPaths}
				actions={
					<Button
						type="button"
						variant="outline"
						size="sm"
						disabled={sourceBusy}
						onClick={() => void runSourceUpdate(() => pickKubeconfigPaths(client))}
					>
						<FilePlus data-icon="inline-start" />
						Choose files
					</Button>
				}
			>
				{sources?.paths.length ? (
					<div className="flex flex-col gap-2">
						{sources.paths.map((entry, index) => (
							<div
								key={entry.path}
								className="flex items-center gap-2 rounded-md border bg-background p-2"
							>
								<div className="min-w-0 flex-1">
									<div className="truncate text-xs font-medium">
										{pathFileName(entry.path)}
									</div>
									<div className="truncate text-xs text-muted-foreground">
										{entry.path}
									</div>
								</div>
								<Button
									type="button"
									variant="ghost"
									size="icon-sm"
									disabled={sourceBusy || index === 0}
									aria-label="Move kubeconfig path up"
									onClick={() => {
										const next = sources.paths.map((path) => path.path);
										[next[index - 1], next[index]] = [next[index], next[index - 1]];
										void runSourceUpdate(() => reorderKubeconfigPaths(client, next));
									}}
								>
									<ArrowUp />
								</Button>
								<Button
									type="button"
									variant="ghost"
									size="icon-sm"
									disabled={sourceBusy || index === sources.paths.length - 1}
									aria-label="Move kubeconfig path down"
									onClick={() => {
										const next = sources.paths.map((path) => path.path);
										[next[index], next[index + 1]] = [next[index + 1], next[index]];
										void runSourceUpdate(() => reorderKubeconfigPaths(client, next));
									}}
								>
									<ArrowDown />
								</Button>
								<Button
									type="button"
									variant="ghost"
									size="icon-sm"
									disabled={sourceBusy}
									aria-label="Remove kubeconfig path"
									onClick={() =>
										void runSourceUpdate(() =>
											removeKubeconfigPath(client, entry.path),
										)
									}
								>
									<Trash2 />
								</Button>
							</div>
						))}
					</div>
				) : (
					<div className="rounded-md border bg-background p-3 text-xs text-muted-foreground">
						No app-added kubeconfig paths.
					</div>
				)}
				<Textarea
					value={pathDraft}
					onChange={(event) => setPathDraft(event.target.value)}
					placeholder="/path/to/kubeconfig.yaml"
					aria-label="Paste kubeconfig paths"
					rows={3}
				/>
				<div className="flex justify-end">
					<Button
						type="button"
						variant="outline"
						size="sm"
						disabled={sourceBusy || !pathDraft.trim()}
						onClick={addPastedPaths}
					>
						<Plus data-icon="inline-start" />
						Add pasted paths
					</Button>
				</div>
			</SettingsBlock>
		</SettingsSection>
	);
}
