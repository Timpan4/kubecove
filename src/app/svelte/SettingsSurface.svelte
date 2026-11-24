<script lang="ts">
	import { onMount } from "svelte";
	import {
		Bot,
		Bug,
		Cable,
		FileCode2,
		FolderCog,
		Palette,
		RefreshCw,
		Search,
		SlidersHorizontal,
		Wrench,
	} from "lucide-svelte";
	import {
		Badge,
		Button,
		buttonClass,
		FieldGroup,
		Input,
		SegmentedControl,
		Switch,
	} from "@/components/ui/svelte";
	import type { LogLayout, TimestampTimezone, YamlDiffStyle } from "@/lib/settings";
	import { USE_BROWSER_DEV_MOCKS } from "@/lib/build-env";
	import { browserMockSurfaces } from "./browserMockSurfaces";
	import {
		getBrowserSessionState,
		subscribeBrowserSession,
		type BrowserSessionState,
	} from "@/lib/browser-runtime";
	import { desktopRemoteConnection } from "@/lib/remote-runtime";
	import type { YamlEncoding, YamlViewMode } from "@/lib/types";
	import McpServerSettings from "./McpServerSettings.svelte";
	import DiagnosticsSettings from "./DiagnosticsSettings.svelte";
	import KubeconfigSettings from "./KubeconfigSettings.svelte";
	import { GENERAL_ROWS, SESSION_ROWS, APPEARANCE_ROWS, YAML_ROWS, KUBECONFIG_ROWS, UPDATES_ROWS, MCP_ROWS, ADVANCED_ROWS, SERVER_INFO_ROW, DIAGNOSTICS_ROWS, type SettingsRowMeta } from "./settingsCatalog";
	import SettingsRow from "./SettingsRow.svelte";
	import { settingsStore } from "@/lib/settings-store";
	import ResourceKindOrderSettings from "./ResourceKindOrderSettings.svelte";
	import AdvancedActionsSettings from "./AdvancedActionsSettings.svelte";

	type SettingsCategoryId =
		| "general"
		| "appearance"
		| "sessions"
		| "yaml"
		| "kubeconfig"
		| "updates"
		| "diagnostics"
		| "advanced"
		| "remote"
		| "agents";

	const WIKI_URL = "https://github.com/Timpan4/kubecove/wiki";

	let browserSession = $state<BrowserSessionState>(getBrowserSessionState());
	const serverInformation = $derived(
		!USE_BROWSER_DEV_MOCKS
			? { version: browserSession.serverVersion, credentialStore: browserSession.credentialStore }
			: $desktopRemoteConnection?.connected
				? {
						version: $desktopRemoteConnection.serverVersion,
						credentialStore: $desktopRemoteConnection.credentialStore,
					}
				: null,
	);

	const categoryRows = $derived({
		general: [
			...Object.values(GENERAL_ROWS),
			...(serverInformation ? [SERVER_INFO_ROW] : []),
		],
		appearance: Object.values(APPEARANCE_ROWS),
		sessions: Object.values(SESSION_ROWS),
		yaml: Object.values(YAML_ROWS),
		kubeconfig: Object.values(KUBECONFIG_ROWS),
		updates: USE_BROWSER_DEV_MOCKS ? Object.values(UPDATES_ROWS) : [],
		diagnostics: DIAGNOSTICS_ROWS,
		advanced: Object.values(ADVANCED_ROWS),
		remote: USE_BROWSER_DEV_MOCKS
			? [{ title: "Remote KubeCove server", description: "Connect this desktop to a headless KubeCove host." }]
			: [],
		agents: USE_BROWSER_DEV_MOCKS ? Object.values(MCP_ROWS) : [],
	} satisfies Record<SettingsCategoryId, SettingsRowMeta[]>);

	const categories: Array<{ id: SettingsCategoryId; label: string }> = [
		{ id: "general", label: "General" },
		{ id: "appearance", label: "Appearance" },
		{ id: "sessions", label: "Live sessions" },
		{ id: "yaml", label: "YAML" },
		{ id: "kubeconfig", label: "Kubeconfig" },
		...(USE_BROWSER_DEV_MOCKS ? [{ id: "updates" as const, label: "Updates" }] : []),
		{ id: "diagnostics", label: "Diagnostics" },
		{ id: "advanced", label: "Advanced" },
		...(USE_BROWSER_DEV_MOCKS ? [
			{ id: "remote" as const, label: "Remote server" },
			{ id: "agents" as const, label: "AI agents" },
		] : []),
	];

	let activeCategory = $state<SettingsCategoryId>("general");
	let query = $state("");
	let { onBack }: { onBack?: () => void } = $props();
	const settings = $derived($settingsStore);

	onMount(() =>
		!USE_BROWSER_DEV_MOCKS
			? subscribeBrowserSession((state) => {
					browserSession = state;
				})
			: undefined,
	);

	const credentialStoreLabel = $derived.by(() => {
		const store = serverInformation?.credentialStore;
		if (!store) return "Not reported";
		const normalized = store.toLowerCase();
		if (normalized.includes("encrypted") || normalized.includes("file")) {
			return "Encrypted file storage";
		}
		if (normalized.includes("native") || normalized.includes("keyring")) {
			return "Native credential storage";
		}
		return store;
	});

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


</script>

<div class="mx-auto grid w-full max-w-7xl gap-5 p-4 md:p-6 xl:grid-cols-[240px_minmax(0,1fr)]">
	<aside class="flex min-w-0 flex-col gap-3">
		{#if onBack}
			<Button
				type="button"
				variant="ghost"
				size="sm"
				class="h-7 w-fit px-0 text-sm text-muted-foreground hover:text-foreground"
				onclick={onBack}
			>
				Back to app
			</Button>
		{/if}
		<div class="relative">
			<Search
				class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
			/>
			<Input
				class="h-9 pl-9 text-sm"
				bind:value={query}
				placeholder="Search settings..."
				aria-label="Search settings"
			/>
		</div>
		<nav class="flex flex-wrap gap-1 xl:grid" aria-label="Settings sections">
			{#each categories as category}
				<Button
					type="button"
					variant={activeCategory === category.id && !searching
						? "secondary"
						: "ghost"}
					size="sm"
					class="h-8 justify-start gap-2.5 px-3 text-sm"
					onclick={() => {
						activeCategory = category.id;
						query = "";
					}}
				>
					{#if category.id === "general"}<SlidersHorizontal data-icon="inline-start" />
					{:else if category.id === "appearance"}<Palette data-icon="inline-start" />
					{:else if category.id === "sessions"}<Cable data-icon="inline-start" />
					{:else if category.id === "yaml"}<FileCode2 data-icon="inline-start" />
					{:else if category.id === "kubeconfig"}<FolderCog data-icon="inline-start" />
					{:else if category.id === "updates"}<RefreshCw data-icon="inline-start" />
					{:else if category.id === "diagnostics"}<Bug data-icon="inline-start" />
					{:else if category.id === "advanced"}<Wrench data-icon="inline-start" />
					{:else if category.id === "agents"}<Bot data-icon="inline-start" />
					{:else}<Cable data-icon="inline-start" />{/if}
					<span>{category.label}</span>
				</Button>
			{/each}
		</nav>
	</aside>

	<section class="flex w-full max-w-5xl min-w-0 flex-col gap-4">
		<h2 class="font-heading text-xl font-semibold">{heading}</h2>

		{#if searching && !hasMatches}
			<div class="rounded-md border bg-card p-4 text-sm text-muted-foreground">
				No settings match "{query.trim()}".
			</div>
		{/if}

		{#if showCategory("general")}
		<FieldGroup>
			<SettingsRow {...GENERAL_ROWS.documentation}>
				<a
					href={WIKI_URL}
					target="_blank"
					rel="noreferrer"
					class={buttonClass({ variant: "outline", size: "sm" })}
				>
					Open Wiki
				</a>
			</SettingsRow>
			{#if serverInformation}
				<SettingsRow {...SERVER_INFO_ROW}>
					<Badge variant="outline">Server {serverInformation.version ?? "Unknown"}</Badge>
					<Badge variant="secondary">{credentialStoreLabel}</Badge>
				</SettingsRow>
			{/if}
			<SettingsRow {...GENERAL_ROWS.exactTimestamps}>
				<Switch
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
				<Switch
					checked={settings.showUsageFooter}
					onCheckedChange={settings.setShowUsageFooter}
					aria-label={GENERAL_ROWS.usageFooter.title}
				/>
			</SettingsRow>
			<SettingsRow {...GENERAL_ROWS.workingSet}>
				<Switch
					checked={settings.measureWorkingSet}
					onCheckedChange={settings.setMeasureWorkingSet}
					disabled={!settings.showUsageFooter}
					aria-label={GENERAL_ROWS.workingSet.title}
				/>
			</SettingsRow>
			<SettingsRow {...GENERAL_ROWS.redactSecrets}>
				<Switch checked={settings.redactSecrets} onCheckedChange={settings.setRedactSecrets} aria-label={GENERAL_ROWS.redactSecrets.title} />
			</SettingsRow>
			<SettingsRow {...GENERAL_ROWS.fullTopologyOnSelection}>
				<Switch
					checked={settings.showFullTopologyOnSelection}
					onCheckedChange={settings.setShowFullTopologyOnSelection}
					aria-label={GENERAL_ROWS.fullTopologyOnSelection.title}
				/>
			</SettingsRow>
			<SettingsRow {...GENERAL_ROWS.ownershipMap}>
				<Switch
					checked={settings.showOwnershipMapByDefault}
					onCheckedChange={settings.setShowOwnershipMapByDefault}
					aria-label={GENERAL_ROWS.ownershipMap.title}
				/>
			</SettingsRow>
			<SettingsRow {...GENERAL_ROWS.timelineExpansion}>
				<Switch
					checked={settings.expandTimelineByDefault}
					onCheckedChange={settings.setExpandTimelineByDefault}
					aria-label={GENERAL_ROWS.timelineExpansion.title}
				/>
			</SettingsRow>
			<SettingsRow {...GENERAL_ROWS.unavailableGitOpsProviders}>
				<Switch
					checked={settings.showUnavailableGitOpsProviders}
					onCheckedChange={settings.setShowUnavailableGitOpsProviders}
					aria-label={GENERAL_ROWS.unavailableGitOpsProviders.title}
				/>
			</SettingsRow>
			<SettingsRow {...GENERAL_ROWS.customResources}>
				<Switch
					checked={settings.showCustomResources}
					onCheckedChange={settings.setShowCustomResources}
					aria-label={GENERAL_ROWS.customResources.title}
				/>
			</SettingsRow>
		</FieldGroup>
		{/if}
		{#if showCategory("appearance")}
		<FieldGroup>
			<SettingsRow {...APPEARANCE_ROWS.logLayout}>
				<SegmentedControl
					value={settings.logLayout}
					options={[
						{ value: "stacked", label: "Stacked" },
						{ value: "gutter", label: "Gutter" },
						{ value: "oneLine", label: "One line" },
					]}
					onChange={(value: LogLayout) => settings.setLogLayout(value)}
					ariaLabel={APPEARANCE_ROWS.logLayout.title}
				/>
			</SettingsRow>
			<ResourceKindOrderSettings {...APPEARANCE_ROWS.resourceKindOrder} />
		</FieldGroup>
		{/if}
		{#if showCategory("sessions")}
		<FieldGroup>
			<SettingsRow {...SESSION_ROWS.autoStartPortForwards}>
				<Switch
					checked={settings.autoStartSavedPortForwards}
					onCheckedChange={settings.setAutoStartSavedPortForwards}
					aria-label={SESSION_ROWS.autoStartPortForwards.title}
				/>
			</SettingsRow>
			<SettingsRow {...SESSION_ROWS.keepLiveSessions}>
				<Switch
					checked={settings.keepLiveSessionsOnWorkspaceSwitch}
					onCheckedChange={settings.setKeepLiveSessionsOnWorkspaceSwitch}
					aria-label={SESSION_ROWS.keepLiveSessions.title}
				/>
			</SettingsRow>
		</FieldGroup>
		{/if}

		{#if showCategory("yaml")}
		<FieldGroup>
			<SettingsRow {...YAML_ROWS.viewMode}>
				<SegmentedControl
					value={settings.yamlViewModeDefault}
					options={[
						{ value: "kubectl", label: "Kubectl view" },
						{ value: "applyClean", label: "Apply clean" },
					]}
					onChange={(value: YamlViewMode) => settings.setYamlViewModeDefault(value)}
					ariaLabel={YAML_ROWS.viewMode.title}
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
				<Switch
					checked={settings.yamlErrorLensEnabled}
					onCheckedChange={settings.setYamlErrorLensEnabled}
					aria-label={YAML_ROWS.errorLens.title}
				/>
			</SettingsRow>
			<SettingsRow {...YAML_ROWS.forceConflicts}>
				<Switch
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

		{#if browserMockSurfaces && showCategory("updates")}
			{#await browserMockSurfaces.updateSettings() then { default: UpdatesSettings }}
				<UpdatesSettings />
			{/await}
		{/if}

		{#if showCategory("diagnostics")}
			<DiagnosticsSettings />
		{/if}

		{#if showCategory("advanced")}
			<AdvancedActionsSettings />
		{/if}

		{#if browserMockSurfaces && showCategory("remote")}
			<FieldGroup>
				<SettingsRow {...categoryRows.remote[0]}>
					{#await browserMockSurfaces.remoteSettings() then { default: RemoteConnectionSettings }}
					<RemoteConnectionSettings />
				{/await}
				</SettingsRow>
			</FieldGroup>
		{/if}

		{#if browserMockSurfaces && showCategory("agents")}
			<McpServerSettings rows={MCP_ROWS} />
		{/if}

	</section>
</div>
