import { useEffect, useState } from "react";
import type { LucideIcon } from "lucide-react";
import {
	Activity,
	ArrowDownToLine,
	ArrowLeft,
	Cable,
	FileCode2,
	FolderCog,
	Search,
	SlidersHorizontal,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ToggleButton } from "@/components/ToggleButton";
import { useSettingsState } from "@/lib/settings";
import { useWorkspaceStore } from "@/lib/workspaces";
import {
	queueUiRuntimeReloadNotice,
	queueUiRuntimeWorkspaceHandoff,
	takeUiRuntimeSettingsFocus,
	type UiRuntimeMode,
	uiRuntimeModeLabel,
} from "@/lib/ui-runtime";
import { cn } from "@/lib/utils";
import { AppUpdatesSection, UPDATES_SETTINGS_ROWS } from "./AppUpdatesSection";
import {
	DIAGNOSTICS_SETTINGS_ROWS,
	DiagnosticsSection,
} from "./DiagnosticsSection";
import {
	KUBECONFIG_SETTINGS_ROWS,
	KubeconfigSourcesSection,
} from "./KubeconfigSourcesSection";
import type { SettingsRowMeta } from "./SettingsControls";
import {
	matchesSettingsQuery,
	SegmentedControl,
	SettingsRow,
	SettingsSearchContext,
	SettingsSection,
} from "./SettingsControls";

type SettingsCategoryId =
	| "general"
	| "sessions"
	| "yaml"
	| "kubeconfig"
	| "updates"
	| "diagnostics";

const GENERAL_ROWS = {
	uiRuntime: {
		title: "Frontend runtime",
		description:
			"Switches between the default Svelte UI and the React fallback. The app reloads after changing mode.",
	},
	exactTimestamps: {
		title: "Show exact timestamps",
		description:
			"Adds the exact timestamp next to relative ages. Exact timestamps remain available in tooltips when this is off.",
	},
	timezone: {
		title: "Timestamp timezone",
		description: "Controls exact timestamps in inline labels and tooltips.",
	},
	usageFooter: {
		title: "Show CPU and memory footer",
		description:
			"Adds a compact footer with KubeCove CPU and memory usage for the app process tree.",
	},
	ownershipMap: {
		title: "Show ownership map by default",
		description:
			"Opens the ownership map when entering resource views. When off, the map stays collapsed until opened.",
	},
	fullTopologyOnSelection: {
		title: "Keep full map visible during selection",
		description:
			"Shows unrelated ownership branches while a resource is selected. Large namespaces may render slower.",
	},
	unavailableGitOpsProviders: {
		title: "Show unavailable GitOps providers",
		description:
			"Shows Argo CD and Flux provider groups as disabled when their CRDs are not detected.",
	},
} satisfies Record<string, SettingsRowMeta>;

const SESSION_ROWS = {
	autoStartPortForwards: {
		title: "Auto-start saved port forwards",
		description:
			"Starts saved Service port-forward presets automatically when a workspace is restored.",
	},
	keepLiveSessions: {
		title: "Keep live sessions across workspace switches",
		description:
			"Leaves port-forward and Pod exec sessions running when you leave their workspace scope. Fixed local port conflicts are skipped and shown in Port Forwards.",
	},
} satisfies Record<string, SettingsRowMeta>;

const YAML_ROWS = {
	cleanupShape: {
		title: "YAML shape",
		description:
			"Controls whether YAML panels open with Kubectl view or Apply clean output.",
	},
	encoding: {
		title: "YAML encoding",
		description:
			"Controls whether YAML panels open as regular YAML or Kubernetes KYAML flow-style text.",
	},
	diffAppearance: {
		title: "YAML diff appearance",
		description: "Controls selected-resource dry-run diff rendering.",
	},
	errorLens: {
		title: "YAML error lens",
		description: "Shows inline editor diagnostics below YAML lines.",
	},
	forceConflicts: {
		title: "Allow YAML force-conflicts",
		description:
			"Lets selected-resource YAML dry-run and apply take server-side field ownership when another manager owns changed fields.",
	},
} satisfies Record<string, SettingsRowMeta>;

const CATEGORIES: ReadonlyArray<{
	id: SettingsCategoryId;
	label: string;
	icon: LucideIcon;
	rows: SettingsRowMeta[];
}> = [
	{
		id: "general",
		label: "General",
		icon: SlidersHorizontal,
		rows: Object.values(GENERAL_ROWS),
	},
	{
		id: "sessions",
		label: "Live sessions",
		icon: Cable,
		rows: Object.values(SESSION_ROWS),
	},
	{ id: "yaml", label: "YAML", icon: FileCode2, rows: Object.values(YAML_ROWS) },
	{
		id: "kubeconfig",
		label: "Kubeconfig",
		icon: FolderCog,
		rows: KUBECONFIG_SETTINGS_ROWS,
	},
	{
		id: "updates",
		label: "Updates",
		icon: ArrowDownToLine,
		rows: UPDATES_SETTINGS_ROWS,
	},
	{
		id: "diagnostics",
		label: "Diagnostics",
		icon: Activity,
		rows: Object.values(DIAGNOSTICS_SETTINGS_ROWS),
	},
];

export function SettingsPage({ onBack }: { onBack: () => void }) {
	const {
		showExactTimestamps,
		showUsageFooter,
		showOwnershipMapByDefault,
		showFullTopologyOnSelection,
		showUnavailableGitOpsProviders,
		uiRuntimeMode,
		autoStartSavedPortForwards,
		keepLiveSessionsOnWorkspaceSwitch,
		allowYamlForceConflicts,
		timestampTimezone,
		yamlViewModeDefault,
		yamlEncodingDefault,
		yamlDiffStyle,
		yamlErrorLensEnabled,
		setShowExactTimestamps,
		setShowUsageFooter,
		setShowOwnershipMapByDefault,
		setShowFullTopologyOnSelection,
		setShowUnavailableGitOpsProviders,
		setUiRuntimeMode,
		setAutoStartSavedPortForwards,
		setKeepLiveSessionsOnWorkspaceSwitch,
		setAllowYamlForceConflicts,
		setTimestampTimezone,
		setYamlViewModeDefault,
		setYamlEncodingDefault,
		setYamlDiffStyle,
		setYamlErrorLensEnabled,
	} = useSettingsState();
	const activeWorkspaceId = useWorkspaceStore((state) => state.activeWorkspaceId);
	const [activeCategory, setActiveCategory] = useState<SettingsCategoryId>("general");
	const [query, setQuery] = useState("");
	const [runtimeSwitchMessage, setRuntimeSwitchMessage] = useState<string | null>(
		null,
	);
	const searching = query.trim().length > 0;
	const hasMatches = CATEGORIES.some((category) =>
		category.rows.some((row) => matchesSettingsQuery(query, row)),
	);
	const showCategory = (id: SettingsCategoryId) =>
		searching || activeCategory === id;
	const heading = searching
		? "Search results"
		: (CATEGORIES.find((category) => category.id === activeCategory)?.label ??
			"Settings");

	useEffect(() => {
		if (!takeUiRuntimeSettingsFocus()) return;
		setActiveCategory("general");
		setQuery(GENERAL_ROWS.uiRuntime.title);
	}, []);

	const handleUiRuntimeModeChange = (mode: UiRuntimeMode) => {
		if (mode === uiRuntimeMode) return;
		if (activeWorkspaceId) queueUiRuntimeWorkspaceHandoff(activeWorkspaceId);
		setUiRuntimeMode(mode);
		queueUiRuntimeReloadNotice(mode);
		setRuntimeSwitchMessage(`Switching to ${uiRuntimeModeLabel(mode)} UI...`);
		window.setTimeout(() => window.location.reload(), 600);
	};

	return (
		<>
		<div className="mx-auto flex w-full max-w-5xl gap-8 pb-8 pt-2">
			<aside className="flex w-52 shrink-0 flex-col gap-4">
				<Button
					type="button"
					variant="ghost"
					size="sm"
					className="justify-start text-muted-foreground"
					onClick={onBack}
				>
					<ArrowLeft data-icon="inline-start" />
					Back to app
				</Button>
				<div className="relative">
					<Search className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
					<Input
						className="h-8 pl-8"
						value={query}
						placeholder="Search settings..."
						aria-label="Search settings"
						onChange={(event) => setQuery(event.target.value)}
					/>
				</div>
				<nav className="flex flex-col gap-1" aria-label="Settings categories">
					{CATEGORIES.map((category) => (
						<button
							key={category.id}
							type="button"
							aria-current={
								!searching && activeCategory === category.id ? "page" : undefined
							}
							className={cn(
								"flex h-8 cursor-pointer items-center gap-2 rounded-md px-2.5 text-sm transition-colors focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-ring/50",
								!searching && activeCategory === category.id
									? "bg-secondary font-medium text-foreground"
									: "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
							)}
							onClick={() => {
								setActiveCategory(category.id);
								setQuery("");
							}}
						>
							<category.icon className="size-4 shrink-0" />
							{category.label}
						</button>
					))}
				</nav>
			</aside>

			<div className="flex min-w-0 flex-1 flex-col gap-8">
				<h1 className="text-xl font-semibold text-foreground">{heading}</h1>
				{searching && !hasMatches && (
					<div className="rounded-xl border bg-card p-6 text-sm text-muted-foreground">
						No settings match "{query.trim()}".
					</div>
				)}
				<SettingsSearchContext.Provider value={query}>
					{showCategory("general") && (
						<SettingsSection title="General" showTitle={searching}>
							<SettingsRow {...GENERAL_ROWS.uiRuntime}>
								<SegmentedControl
									value={uiRuntimeMode}
									options={[
										{ value: "react", label: "React" },
										{ value: "svelte", label: "Svelte" },
									]}
									onChange={handleUiRuntimeModeChange}
									ariaLabel={GENERAL_ROWS.uiRuntime.title}
								/>
							</SettingsRow>
							<SettingsRow {...GENERAL_ROWS.exactTimestamps}>
								<ToggleButton
									checked={showExactTimestamps}
									onCheckedChange={setShowExactTimestamps}
									ariaLabel={GENERAL_ROWS.exactTimestamps.title}
								/>
							</SettingsRow>
							<SettingsRow {...GENERAL_ROWS.timezone}>
								<SegmentedControl
									value={timestampTimezone}
									options={[
										{ value: "local", label: "Local" },
										{ value: "utc", label: "UTC" },
									]}
									onChange={setTimestampTimezone}
									ariaLabel={GENERAL_ROWS.timezone.title}
								/>
							</SettingsRow>
							<SettingsRow {...GENERAL_ROWS.usageFooter}>
								<ToggleButton
									checked={showUsageFooter}
									onCheckedChange={setShowUsageFooter}
									ariaLabel={GENERAL_ROWS.usageFooter.title}
								/>
							</SettingsRow>
							<SettingsRow {...GENERAL_ROWS.ownershipMap}>
							<ToggleButton
								checked={showOwnershipMapByDefault}
								onCheckedChange={setShowOwnershipMapByDefault}
								ariaLabel={GENERAL_ROWS.ownershipMap.title}
							/>
						</SettingsRow>
						<SettingsRow {...GENERAL_ROWS.fullTopologyOnSelection}>
							<ToggleButton
								checked={showFullTopologyOnSelection}
								onCheckedChange={setShowFullTopologyOnSelection}
								ariaLabel={GENERAL_ROWS.fullTopologyOnSelection.title}
							/>
						</SettingsRow>
						<SettingsRow {...GENERAL_ROWS.unavailableGitOpsProviders}>
							<ToggleButton
								checked={showUnavailableGitOpsProviders}
								onCheckedChange={setShowUnavailableGitOpsProviders}
								ariaLabel={GENERAL_ROWS.unavailableGitOpsProviders.title}
							/>
						</SettingsRow>
					</SettingsSection>
				)}

					{showCategory("sessions") && (
						<SettingsSection title="Live sessions" showTitle={searching}>
							<SettingsRow {...SESSION_ROWS.autoStartPortForwards}>
								<ToggleButton
									checked={autoStartSavedPortForwards}
									onCheckedChange={setAutoStartSavedPortForwards}
									ariaLabel={SESSION_ROWS.autoStartPortForwards.title}
								/>
							</SettingsRow>
							<SettingsRow {...SESSION_ROWS.keepLiveSessions}>
								<ToggleButton
									checked={keepLiveSessionsOnWorkspaceSwitch}
									onCheckedChange={setKeepLiveSessionsOnWorkspaceSwitch}
									ariaLabel={SESSION_ROWS.keepLiveSessions.title}
								/>
							</SettingsRow>
						</SettingsSection>
					)}

					{showCategory("yaml") && (
						<SettingsSection title="YAML" showTitle={searching}>
							<SettingsRow {...YAML_ROWS.cleanupShape}>
								<SegmentedControl
									value={yamlViewModeDefault}
									options={[
										{ value: "kubectl", label: "Kubectl view" },
										{ value: "applyClean", label: "Apply clean" },
									]}
									onChange={setYamlViewModeDefault}
									ariaLabel={YAML_ROWS.cleanupShape.title}
								/>
							</SettingsRow>
							<SettingsRow {...YAML_ROWS.encoding}>
								<SegmentedControl
									value={yamlEncodingDefault}
									options={[
										{ value: "yaml", label: "YAML" },
										{ value: "kyaml", label: "KYAML" },
									]}
									onChange={setYamlEncodingDefault}
									ariaLabel={YAML_ROWS.encoding.title}
								/>
							</SettingsRow>
							<SettingsRow {...YAML_ROWS.diffAppearance}>
								<SegmentedControl
									value={yamlDiffStyle}
									options={[
										{ value: "clean", label: "Clean" },
										{ value: "git", label: "Git" },
									]}
									onChange={setYamlDiffStyle}
									ariaLabel={YAML_ROWS.diffAppearance.title}
								/>
							</SettingsRow>
							<SettingsRow {...YAML_ROWS.errorLens}>
								<ToggleButton
									checked={yamlErrorLensEnabled}
									onCheckedChange={setYamlErrorLensEnabled}
									ariaLabel={YAML_ROWS.errorLens.title}
								/>
							</SettingsRow>
							<SettingsRow {...YAML_ROWS.forceConflicts}>
								<ToggleButton
									checked={allowYamlForceConflicts}
									onCheckedChange={setAllowYamlForceConflicts}
									ariaLabel={YAML_ROWS.forceConflicts.title}
								/>
							</SettingsRow>
						</SettingsSection>
					)}

					{showCategory("kubeconfig") && (
						<KubeconfigSourcesSection showTitle={searching} />
					)}

					{showCategory("updates") && <AppUpdatesSection showTitle={searching} />}

					{showCategory("diagnostics") && (
						<DiagnosticsSection showTitle={searching} />
					)}
				</SettingsSearchContext.Provider>
			</div>
		</div>
		{runtimeSwitchMessage && (
			<div
				role="status"
				className="fixed bottom-4 left-1/2 z-50 -translate-x-1/2 rounded-md border bg-popover px-3 py-2 text-sm text-popover-foreground shadow-lg"
			>
				{runtimeSwitchMessage}
			</div>
		)}
		</>
	);
}
