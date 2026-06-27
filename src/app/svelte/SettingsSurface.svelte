<script lang="ts">
	import { onMount } from "svelte";
	import {
		Activity,
		Bug,
		Cable,
		FileCode2,
		FolderCog,
		RefreshCw,
		Search,
		Settings,
		SlidersHorizontal,
	} from "lucide-svelte";
	import { createQuery } from "@tanstack/svelte-query";
	import {
		Button,
		Checkbox,
		FieldGroup,
		Input,
		SegmentedControl,
	} from "@/components/ui/svelte";
	import { queryKeys } from "@/lib/queryKeys";
	import type { TimestampTimezone, YamlDiffStyle } from "@/lib/settings";
	import {
		takeUiRuntimeSettingsFocus,
	} from "@/lib/ui-runtime";
	import {
		createTauriClient,
		getAppUsageMetrics,
	} from "@/lib/tauri";
	import type { AppUsageMetrics, YamlEncoding, YamlViewMode } from "@/lib/types";
	import DiagnosticsSettings from "./DiagnosticsSettings.svelte";
	import KubeconfigSettings from "./KubeconfigSettings.svelte";
	import SettingsRow from "./SettingsRow.svelte";
	import StatGrid from "./StatGrid.svelte";
	import SurfaceFrame from "./SurfaceFrame.svelte";
	import UpdatesSettings from "./UpdatesSettings.svelte";
	import { settingsStore } from "@/lib/settings-store";

	type SettingsCategoryId =
		| "general"
		| "sessions"
		| "yaml"
		| "kubeconfig"
		| "updates"
		| "diagnostics";
	type SettingsRowMeta = { title: string; description: string };

	const GENERAL_ROWS = {
		exactTimestamps: {
			title: "Show exact timestamps",
			description: "Adds exact timestamps next to relative ages.",
		},
		timezone: {
			title: "Timestamp timezone",
			description: "Controls exact timestamps in labels and tooltips.",
		},
		usageFooter: {
			title: "Show CPU and memory footer",
			description: "Shows compact app process CPU and memory usage.",
		},
		ownershipMap: {
			title: "Show ownership map by default",
			description: "Opens the ownership map when entering resource views.",
		},
		fullTopologyOnSelection: {
			title: "Keep full map visible during selection",
			description:
				"Shows unrelated ownership branches while a resource is selected. Large namespaces may render slower.",
		},
		unavailableGitOpsProviders: {
			title: "Show unavailable GitOps providers",
			description:
				"Shows Argo CD and Flux provider groups as disabled when CRDs are missing.",
		},
	} satisfies Record<string, SettingsRowMeta>;

	const SESSION_ROWS = {
		autoStartPortForwards: {
			title: "Auto-start saved port forwards",
			description:
				"Starts saved Service port-forward presets when a workspace is restored.",
		},
		keepLiveSessions: {
			title: "Keep live sessions across workspace switches",
			description:
				"Leaves port-forward and Pod exec sessions running when you leave their scope.",
		},
	} satisfies Record<string, SettingsRowMeta>;

	const YAML_ROWS = {
		cleanupShape: {
			title: "YAML shape",
			description:
				"Controls whether YAML opens as Kubectl view or Apply clean output.",
		},
		encoding: {
			title: "YAML encoding",
			description: "Controls whether YAML panels open as regular YAML or KYAML.",
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
			description: "Lets guarded YAML operations take server-side field ownership.",
		},
	} satisfies Record<string, SettingsRowMeta>;

	const KUBECONFIG_ROWS = {
		activeSources: {
			title: "Active sources",
			description:
				"Shows where contexts are discovered from. Full file paths are shown only here.",
		},
		envVar: {
			title: "Environment variable",
			description:
				"Variable holding kubeconfig paths. The default path is read when it is unset.",
		},
		sourceLabels: {
			title: "Show source labels",
			description:
				"Marks contexts with the kubeconfig source they were discovered from.",
		},
		addedPaths: {
			title: "Added kubeconfig paths",
			description:
				"Extra kubeconfig files KubeCove reads alongside the environment variable.",
		},
	} satisfies Record<string, SettingsRowMeta>;

	const UPDATES_ROWS = {
		version: {
			title: "KubeCove version",
			description: "Current version, update checks, and installing new releases.",
		},
	} satisfies Record<string, SettingsRowMeta>;

	const DIAGNOSTICS_ROWS = {
		debugMode: {
			title: "Enable diagnostics",
			description:
				"Collects local latency timings for release smoke testing. Trace data stays in memory.",
		},
		report: {
			title: "Latency report",
			description:
				"Shows frontend and backend timing summaries, with redacted copy output by default.",
		},
		topologySpike: {
			title: "Topology spike",
			description:
				"Opens the Svelte topology benchmark harness with synthetic 4,000-node data, compact nodes, selected edges, and a focused viewport.",
		},
	} satisfies Record<string, SettingsRowMeta>;

	const categoryRows: Record<SettingsCategoryId, SettingsRowMeta[]> = {
		general: Object.values(GENERAL_ROWS),
		sessions: Object.values(SESSION_ROWS),
		yaml: Object.values(YAML_ROWS),
		kubeconfig: Object.values(KUBECONFIG_ROWS),
		updates: Object.values(UPDATES_ROWS),
		diagnostics: Object.values(DIAGNOSTICS_ROWS),
	};

	const categories: Array<{ id: SettingsCategoryId; label: string }> = [
		{ id: "general", label: "General" },
		{ id: "sessions", label: "Live sessions" },
		{ id: "yaml", label: "YAML" },
		{ id: "kubeconfig", label: "Kubeconfig" },
		{ id: "updates", label: "Updates" },
		{ id: "diagnostics", label: "Diagnostics" },
	];

	const client = createTauriClient();
	let activeCategory = $state<SettingsCategoryId>("general");
	let query = $state("");
	const settings = $derived($settingsStore);

	const searching = $derived(query.trim().length > 0);
	const heading = $derived(
		searching
			? "Search results"
			: (categories.find((category) => category.id === activeCategory)?.label ??
				"Settings"),
	);
	const hasMatches = $derived(
		Object.values(categoryRows)
			.flat()
			.some((row) => matchesSettingsQuery(query, row)),
	);

	const usageQuery = createQuery<AppUsageMetrics>(() => ({
		queryKey: queryKeys.appUsageMetrics(),
		queryFn: () => getAppUsageMetrics(client),
		enabled: settings.showUsageFooter,
		placeholderData: (previousData) => previousData,
		refetchInterval: 5_000,
	}));

	onMount(() => {
		if (!takeUiRuntimeSettingsFocus()) return;
		activeCategory = "general";
		query = "";
	});

	function matchesSettingsQuery(value: string, row: SettingsRowMeta): boolean {
		const needle = value.trim().toLowerCase();
		return (
			needle.length === 0 ||
			row.title.toLowerCase().includes(needle) ||
			row.description.toLowerCase().includes(needle)
		);
	}

	function showCategory(id: SettingsCategoryId): boolean {
		if (!searching) return activeCategory === id;
		return categoryRows[id].some((row) => matchesSettingsQuery(query, row));
	}

	function memoryText(bytes: number): string {
		if (bytes >= 1024 * 1024 * 1024) {
			return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
		}
		if (bytes >= 1024 * 1024) {
			return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
		}
		return `${Math.round(bytes / 1024)} KB`;
	}
</script>

<div class="mx-auto flex w-full max-w-5xl flex-col gap-4 p-4 md:p-6">
	<header class="flex flex-wrap items-center justify-between gap-3">
		<div class="flex items-center gap-3">
			<Settings class="size-5 text-muted-foreground" />
			<div>
				<h2 class="font-heading text-lg font-semibold">Settings</h2>
				<p class="text-xs text-muted-foreground">
					Runtime, safety, live-session, and YAML preferences.
				</p>
			</div>
		</div>
		<div class="relative w-full sm:w-64">
			<Search
				class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
			/>
			<Input
				class="h-8 pl-8"
				bind:value={query}
				placeholder="Search settings..."
				aria-label="Search settings"
			/>
		</div>
	</header>

	<div class="flex flex-wrap gap-2">
		{#each categories as category}
			<Button
				type="button"
				variant={activeCategory === category.id && !searching
					? "secondary"
					: "outline"}
				size="sm"
				onclick={() => {
					activeCategory = category.id;
					query = "";
				}}
			>
				{category.label}
			</Button>
		{/each}
	</div>

	<StatGrid
		stats={[
			["Runtime", "Svelte"],
			["Usage footer", settings.showUsageFooter ? "on" : "off"],
			["YAML mode", settings.yamlViewModeDefault],
			["Kubeconfig", settings.kubeconfigSourceLabel],
		]}
	/>

	{#if searching && !hasMatches}
		<div class="rounded-md border bg-card p-4 text-sm text-muted-foreground">
			No settings match "{query.trim()}".
		</div>
	{/if}

	<div class="flex items-center gap-2 text-xs font-medium text-muted-foreground">
		{#if heading === "General"}<SlidersHorizontal class="size-3.5" />
		{:else if heading === "Live sessions"}<Cable class="size-3.5" />
		{:else if heading === "YAML"}<FileCode2 class="size-3.5" />
		{:else if heading === "Kubeconfig"}<FolderCog class="size-3.5" />
		{:else if heading === "Updates"}<RefreshCw class="size-3.5" />
		{:else}<Bug class="size-3.5" />{/if}
		<span>{heading}</span>
	</div>

	{#if showCategory("general")}
		<FieldGroup>
			<SettingsRow {...GENERAL_ROWS.exactTimestamps}>
				<Checkbox
					checked={settings.showExactTimestamps}
					onCheckedChange={settings.setShowExactTimestamps}
					aria-label={GENERAL_ROWS.exactTimestamps.title}
				/>
			</SettingsRow>
			<SettingsRow {...GENERAL_ROWS.timezone}>
				<SegmentedControl
					value={settings.timestampTimezone}
					options={[
						{ value: "local", label: "Local" },
						{ value: "utc", label: "UTC" },
					]}
					onChange={(value: TimestampTimezone) =>
						settings.setTimestampTimezone(value)}
					ariaLabel={GENERAL_ROWS.timezone.title}
				/>
			</SettingsRow>
			<SettingsRow {...GENERAL_ROWS.usageFooter}>
				<Checkbox
					checked={settings.showUsageFooter}
					onCheckedChange={settings.setShowUsageFooter}
					aria-label={GENERAL_ROWS.usageFooter.title}
				/>
			</SettingsRow>
			<SettingsRow {...GENERAL_ROWS.ownershipMap}>
				<Checkbox
					checked={settings.showOwnershipMapByDefault}
					onCheckedChange={settings.setShowOwnershipMapByDefault}
					aria-label={GENERAL_ROWS.ownershipMap.title}
				/>
			</SettingsRow>
			<SettingsRow {...GENERAL_ROWS.fullTopologyOnSelection}>
				<Checkbox
					checked={settings.showFullTopologyOnSelection}
					onCheckedChange={settings.setShowFullTopologyOnSelection}
					aria-label={GENERAL_ROWS.fullTopologyOnSelection.title}
				/>
			</SettingsRow>
			<SettingsRow {...GENERAL_ROWS.unavailableGitOpsProviders}>
				<Checkbox
					checked={settings.showUnavailableGitOpsProviders}
					onCheckedChange={settings.setShowUnavailableGitOpsProviders}
					aria-label={GENERAL_ROWS.unavailableGitOpsProviders.title}
				/>
			</SettingsRow>
		</FieldGroup>
	{/if}

	{#if showCategory("sessions")}
		<FieldGroup>
			<SettingsRow {...SESSION_ROWS.autoStartPortForwards}>
				<Checkbox
					checked={settings.autoStartSavedPortForwards}
					onCheckedChange={settings.setAutoStartSavedPortForwards}
					aria-label={SESSION_ROWS.autoStartPortForwards.title}
				/>
			</SettingsRow>
			<SettingsRow {...SESSION_ROWS.keepLiveSessions}>
				<Checkbox
					checked={settings.keepLiveSessionsOnWorkspaceSwitch}
					onCheckedChange={settings.setKeepLiveSessionsOnWorkspaceSwitch}
					aria-label={SESSION_ROWS.keepLiveSessions.title}
				/>
			</SettingsRow>
		</FieldGroup>
	{/if}

	{#if showCategory("yaml")}
		<FieldGroup>
			<SettingsRow {...YAML_ROWS.cleanupShape}>
				<SegmentedControl
					value={settings.yamlViewModeDefault}
					options={[
						{ value: "kubectl", label: "Kubectl view" },
						{ value: "applyClean", label: "Apply clean" },
					]}
					onChange={(value: YamlViewMode) => settings.setYamlViewModeDefault(value)}
					ariaLabel={YAML_ROWS.cleanupShape.title}
				/>
			</SettingsRow>
			<SettingsRow {...YAML_ROWS.encoding}>
				<SegmentedControl
					value={settings.yamlEncodingDefault}
					options={[
						{ value: "yaml", label: "YAML" },
						{ value: "kyaml", label: "KYAML" },
					]}
					onChange={(value: YamlEncoding) => settings.setYamlEncodingDefault(value)}
					ariaLabel={YAML_ROWS.encoding.title}
				/>
			</SettingsRow>
			<SettingsRow {...YAML_ROWS.diffAppearance}>
				<SegmentedControl
					value={settings.yamlDiffStyle}
					options={[
						{ value: "clean", label: "Clean" },
						{ value: "git", label: "Git" },
					]}
					onChange={(value: YamlDiffStyle) => settings.setYamlDiffStyle(value)}
					ariaLabel={YAML_ROWS.diffAppearance.title}
				/>
			</SettingsRow>
			<SettingsRow {...YAML_ROWS.errorLens}>
				<Checkbox
					checked={settings.yamlErrorLensEnabled}
					onCheckedChange={settings.setYamlErrorLensEnabled}
					aria-label={YAML_ROWS.errorLens.title}
				/>
			</SettingsRow>
			<SettingsRow {...YAML_ROWS.forceConflicts}>
				<Checkbox
					checked={settings.allowYamlForceConflicts}
					onCheckedChange={settings.setAllowYamlForceConflicts}
					aria-label={YAML_ROWS.forceConflicts.title}
				/>
			</SettingsRow>
		</FieldGroup>
	{/if}

	{#if showCategory("kubeconfig")}
		<KubeconfigSettings />
	{/if}

	{#if showCategory("updates")}
		<UpdatesSettings />
	{/if}

	{#if showCategory("diagnostics")}
		<DiagnosticsSettings />
	{/if}

	{#if settings.showUsageFooter}
		<SurfaceFrame
			icon={Activity}
			title="Usage Footer"
			query={usageQuery}
			errorLabel="Usage metrics unavailable"
		>
			{@const usage = usageQuery.data}
			{#if usage}
				<StatGrid
					stats={[
						["CPU", `${usage.cpuPercent.toFixed(1)}%`],
						["Memory", memoryText(usage.memoryBytes)],
						["Processes", usage.processCount],
					]}
				/>
			{/if}
		</SurfaceFrame>
	{/if}
</div>
