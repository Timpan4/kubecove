import type { ReactNode } from "react";
import { useEffect, useMemo, useState } from "react";
import {
	ArrowDown,
	ArrowUp,
	Check,
	Download,
	FilePlus,
	Plus,
	RefreshCw,
	RotateCcw,
	Trash2,
} from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { ToggleButton } from "@/components/ToggleButton";
import { useAppUpdateStore } from "@/features/app-updates";
import {
	DEFAULT_KUBECONFIG_ENV_VAR,
	type TimestampTimezone,
	useSettingsState,
} from "@/lib/settings";
import type { YamlEncoding, YamlViewMode } from "@/lib/types";
import { isAppUpdatesEnabled } from "@/lib/release-channel";
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

const CHECKED_AT_FORMATTER = new Intl.DateTimeFormat(undefined, {
	dateStyle: "medium",
	timeStyle: "short",
});

function SettingsRow({
	title,
	description,
	children,
}: {
	title: string;
	description: string;
	children: ReactNode;
}) {
	return (
		<div className="flex min-h-16 items-center justify-between gap-6 border-b py-4">
			<div className="min-w-0">
				<div className="text-sm font-medium text-foreground">{title}</div>
				<div className="mt-1 max-w-2xl text-xs leading-relaxed text-muted-foreground">
					{description}
				</div>
			</div>
			<div className="shrink-0">{children}</div>
		</div>
	);
}

function TimezoneOption({
	value,
	label,
	selected,
	onSelect,
}: {
	value: TimestampTimezone;
	label: string;
	selected: boolean;
	onSelect: (value: TimestampTimezone) => void;
}) {
	return (
		<Button
			type="button"
			variant={selected ? "secondary" : "ghost"}
			size="sm"
			className="h-8 rounded-sm px-2.5"
			onClick={() => onSelect(value)}
			aria-pressed={selected}
		>
			{selected && <Check data-icon="inline-start" />}
			{label}
		</Button>
	);
}

function YamlEncodingOption({
	value,
	label,
	selected,
	onSelect,
}: {
	value: YamlEncoding;
	label: string;
	selected: boolean;
	onSelect: (value: YamlEncoding) => void;
}) {
	return (
		<Button
			type="button"
			variant={selected ? "secondary" : "ghost"}
			size="sm"
			className="h-8 rounded-sm px-2.5"
			onClick={() => onSelect(value)}
			aria-pressed={selected}
		>
			{selected && <Check data-icon="inline-start" />}
			{label}
		</Button>
	);
}

function YamlViewModeOption({
	value,
	label,
	selected,
	onSelect,
}: {
	value: YamlViewMode;
	label: string;
	selected: boolean;
	onSelect: (value: YamlViewMode) => void;
}) {
	return (
		<Button
			type="button"
			variant={selected ? "secondary" : "ghost"}
			size="sm"
			className="h-8 rounded-sm px-2.5"
			onClick={() => onSelect(value)}
			aria-pressed={selected}
		>
			{selected && <Check data-icon="inline-start" />}
			{label}
		</Button>
	);
}

function formatCheckedAt(value: string | null): string {
	if (!value) return "Not checked yet";
	const date = new Date(value);
	if (Number.isNaN(date.getTime())) return "Not checked yet";
	return CHECKED_AT_FORMATTER.format(date);
}

function pathFileName(path: string): string {
	return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

function sourceDescription(sources: KubeconfigSourcesSummary | null): string {
	if (!sources) return "Loading kubeconfig sources.";
	return `${sources.sourceLabel}. Full file paths are shown only here.`;
}

export function SettingsPage() {
	const {
		showExactTimestamps,
		showUsageFooter,
		autoStartSavedPortForwards,
		keepLiveSessionsOnWorkspaceSwitch,
		timestampTimezone,
		yamlViewModeDefault,
		yamlEncodingDefault,
		kubeconfigEnvVar,
		showKubeconfigSourceLabels,
		setShowExactTimestamps,
		setShowUsageFooter,
		setAutoStartSavedPortForwards,
		setKeepLiveSessionsOnWorkspaceSwitch,
		setTimestampTimezone,
		setYamlViewModeDefault,
		setYamlEncodingDefault,
		setKubeconfigEnvVar,
		resetKubeconfigEnvVar,
		setKubeconfigSources,
	} = useSettingsState();
	const {
		status,
		currentVersion,
		availableVersion,
		downloadProgress,
		lastCheckedAt,
		errorMessage,
		checkForUpdates,
		installUpdate,
		relaunchApp,
	} = useAppUpdateStore();
	const updatesEnabled = isAppUpdatesEnabled();
	const updateBusy = status === "checking" || status === "downloading";
	const client = useMemo(() => createTauriClient(), []);
	const [sources, setSources] = useState<KubeconfigSourcesSummary | null>(null);
	const [envDraft, setEnvDraft] = useState(kubeconfigEnvVar);
	const [pathDraft, setPathDraft] = useState("");
	const [sourceBusy, setSourceBusy] = useState(false);
	const [sourceError, setSourceError] = useState<string | null>(null);

	const applySources = (next: KubeconfigSourcesSummary) => {
		setSources(next);
		setKubeconfigSources(next);
		setEnvDraft(next.kubeconfigEnvVar);
		setSourceError(null);
	};

	useEffect(() => {
		void getKubeconfigSources(client)
			.then(applySources)
			.catch((error) =>
				setSourceError(error instanceof Error ? error.message : String(error)),
			);
	}, [client]);

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
		<div className="mx-auto flex w-full max-w-4xl flex-col gap-6">
			<header className="border-b pb-4">
				<h1 className="text-lg font-semibold text-foreground">Settings</h1>
			</header>

			<section className="flex flex-col">
				<SettingsRow
					title="Show exact timestamps"
					description="Adds the exact timestamp next to relative ages. Exact timestamps remain available in tooltips when this is off."
				>
					<ToggleButton
						checked={showExactTimestamps}
						onCheckedChange={setShowExactTimestamps}
						ariaLabel="Show exact timestamps"
					/>
				</SettingsRow>
				<SettingsRow
					title="Show CPU and memory footer"
					description="Adds a compact footer with KubeCove CPU and memory usage for the app process tree."
				>
					<ToggleButton
						checked={showUsageFooter}
						onCheckedChange={setShowUsageFooter}
						ariaLabel="Show CPU and memory footer"
					/>
				</SettingsRow>
				<SettingsRow
					title="Auto-start saved port forwards"
					description="Starts saved Service port-forward presets automatically when a workspace is restored."
				>
					<ToggleButton
						checked={autoStartSavedPortForwards}
						onCheckedChange={setAutoStartSavedPortForwards}
						ariaLabel="Auto-start saved port forwards"
					/>
				</SettingsRow>
				<SettingsRow
					title="Keep live sessions across workspace switches"
					description="Leaves port-forward and Pod exec sessions running when you leave their workspace scope. Fixed local port conflicts are skipped and shown in Port Forwards."
				>
					<ToggleButton
						checked={keepLiveSessionsOnWorkspaceSwitch}
						onCheckedChange={setKeepLiveSessionsOnWorkspaceSwitch}
						ariaLabel="Keep live sessions across workspace switches"
					/>
				</SettingsRow>
				<div className="border-b py-4">
					<div className="flex flex-col gap-4">
						<div className="flex items-start justify-between gap-6">
							<div className="min-w-0">
								<div className="text-sm font-medium text-foreground">
									Kubeconfig sources
								</div>
								<div className="mt-1 max-w-2xl text-xs leading-relaxed text-muted-foreground">
									{sourceDescription(sources)}
								</div>
							</div>
							<Badge variant="outline">
								{sources?.sourceLabel ?? DEFAULT_KUBECONFIG_ENV_VAR}
							</Badge>
						</div>

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

						<div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
							<div className="flex min-w-0 items-center gap-2">
								<Input
									className="h-8"
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
							<div className="flex items-center gap-2">
								<span className="text-xs text-muted-foreground">Show labels</span>
								<ToggleButton
									checked={showKubeconfigSourceLabels}
									onCheckedChange={(show) =>
										void runSourceUpdate(() =>
											setShowKubeconfigSourceLabels(client, show),
										)
									}
									ariaLabel="Show kubeconfig source labels"
								/>
							</div>
						</div>

						<div className="flex flex-col gap-2">
							<div className="flex items-center justify-between gap-3">
								<div className="text-xs font-medium text-foreground">
									Added kubeconfig paths
								</div>
								<Button
									type="button"
									variant="outline"
									size="sm"
									disabled={sourceBusy}
									onClick={() =>
										void runSourceUpdate(() => pickKubeconfigPaths(client))
									}
								>
									<FilePlus data-icon="inline-start" />
									Choose files
								</Button>
							</div>
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
													[next[index - 1], next[index]] = [
														next[index],
														next[index - 1],
													];
													void runSourceUpdate(() =>
														reorderKubeconfigPaths(client, next),
													);
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
													[next[index], next[index + 1]] = [
														next[index + 1],
														next[index],
													];
													void runSourceUpdate(() =>
														reorderKubeconfigPaths(client, next),
													);
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
						</div>

						<div className="flex flex-col gap-2">
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
						</div>
					</div>
				</div>
				<SettingsRow
					title="Timestamp timezone"
					description="Controls exact timestamps in inline labels and tooltips."
				>
					<div className="flex rounded-md border bg-background/50 p-0.5">
						<TimezoneOption
							value="local"
							label="Local"
							selected={timestampTimezone === "local"}
							onSelect={setTimestampTimezone}
						/>
						<TimezoneOption
							value="utc"
							label="UTC"
							selected={timestampTimezone === "utc"}
							onSelect={setTimestampTimezone}
						/>
					</div>
				</SettingsRow>
				<SettingsRow
					title="YAML cleanup shape"
					description="Controls whether YAML panels open with kubectl-style inspect output or apply-friendly output."
				>
					<div className="flex rounded-md border bg-background/50 p-0.5">
						<YamlViewModeOption
							value="kubectl"
							label="Kubectl"
							selected={yamlViewModeDefault === "kubectl"}
							onSelect={setYamlViewModeDefault}
						/>
						<YamlViewModeOption
							value="applyClean"
							label="Apply-friendly"
							selected={yamlViewModeDefault === "applyClean"}
							onSelect={setYamlViewModeDefault}
						/>
					</div>
				</SettingsRow>
				<SettingsRow
					title="YAML encoding"
					description="Controls whether YAML panels open as regular YAML or Kubernetes KYAML flow-style text."
				>
					<div className="flex rounded-md border bg-background/50 p-0.5">
						<YamlEncodingOption
							value="yaml"
							label="YAML"
							selected={yamlEncodingDefault === "yaml"}
							onSelect={setYamlEncodingDefault}
						/>
						<YamlEncodingOption
							value="kyaml"
							label="KYAML"
							selected={yamlEncodingDefault === "kyaml"}
							onSelect={setYamlEncodingDefault}
						/>
					</div>
				</SettingsRow>
				<SettingsRow
					title="KubeCove version"
					description={
						updatesEnabled
							? `Current version ${currentVersion}. Last checked: ${formatCheckedAt(lastCheckedAt)}.`
							: `Current version ${currentVersion}. Development build; update checks disabled.`
					}
				>
					<div className="flex items-center gap-2">
						{updatesEnabled && availableVersion && (
							<Badge variant="secondary">Update {availableVersion}</Badge>
						)}
						{updatesEnabled ? (
							<Button
								type="button"
								variant="outline"
								size="sm"
								disabled={updateBusy}
								onClick={() => void checkForUpdates({ manual: true })}
							>
								<RefreshCw data-icon="inline-start" />
								Check
							</Button>
						) : (
							<Badge variant="outline">Dev build</Badge>
						)}
					</div>
				</SettingsRow>
				{updatesEnabled && status === "available" && (
					<SettingsRow
						title="Update available"
						description={`KubeCove ${availableVersion} can be downloaded and installed now.`}
					>
						<Button type="button" size="sm" onClick={() => void installUpdate()}>
							<Download data-icon="inline-start" />
							Install
						</Button>
					</SettingsRow>
				)}
				{updatesEnabled && status === "downloading" && (
					<SettingsRow
						title="Installing update"
						description={
							downloadProgress === null
								? "Preparing the installer."
								: `Download progress: ${downloadProgress}%.`
						}
					>
						<Badge variant="secondary">Working</Badge>
					</SettingsRow>
				)}
				{updatesEnabled && status === "installed" && (
					<SettingsRow
						title="Relaunch required"
						description="The update is installed. Relaunch KubeCove to finish."
					>
						<Button type="button" size="sm" onClick={() => void relaunchApp()}>
							<RefreshCw data-icon="inline-start" />
							Relaunch
						</Button>
					</SettingsRow>
				)}
				{updatesEnabled && status === "error" && errorMessage && (
					<SettingsRow
						title="Update check failed"
						description={errorMessage}
					>
						<Button
							type="button"
							variant="outline"
							size="sm"
							onClick={() => void checkForUpdates({ manual: true })}
						>
							<RefreshCw data-icon="inline-start" />
							Retry
						</Button>
					</SettingsRow>
				)}
			</section>
		</div>
	);
}
