<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import {
		ArrowDown,
		ArrowUp,
		FilePlus,
		Plus,
		RotateCcw,
		Trash2,
	} from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Badge,
		Button,
		FieldGroup,
		Input,
		Switch,
		Textarea,
	} from "@/components/ui/svelte";
	import {
		DEFAULT_KUBECONFIG_ENV_VAR,
	} from "@/lib/settings";
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
	import SettingsRow from "./SettingsRow.svelte";
	import { getSettingsSnapshot, settingsStore } from "@/lib/settings-store";

	const KUBECONFIG_SOURCES_QUERY_KEY = ["kubeconfig-sources"] as const;
	const client = createTauriClient();
	const queryClient = useQueryClient();
	let envDraft = $state(getSettingsSnapshot().kubeconfigEnvVar);
	let pathDraft = $state("");
	let sourceActionBusy = $state(false);
	let sourceActionError = $state<unknown>(null);
	const settings = $derived($settingsStore);
	const sourceQuery = createQuery<KubeconfigSourcesSummary>(() => ({
		queryKey: KUBECONFIG_SOURCES_QUERY_KEY,
		queryFn: () => getKubeconfigSources(client),
		staleTime: 60_000,
	}));
	const sources = $derived(sourceQuery.data ?? null);
	const sourceBusy = $derived(sourceActionBusy || sourceQuery.isFetching);
	const sourceError = $derived(
		sourceActionError ??
			(sourceQuery.isError ? sourceQuery.error : null),
	);

	$effect(() => {
		if (!sourceBusy) envDraft = settings.kubeconfigEnvVar;
	});

	function pathFileName(path: string): string {
		return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
	}

	function sourceDescription(): string {
		if (!sources) return "Loading kubeconfig sources.";
		return `${sources.sourceLabel}. Full file paths are shown only here.`;
	}

	function applySources(next: KubeconfigSourcesSummary) {
		queryClient.setQueryData(KUBECONFIG_SOURCES_QUERY_KEY, next);
		getSettingsSnapshot().setKubeconfigSources(next);
		envDraft = next.kubeconfigEnvVar;
		sourceActionError = null;
	}

	async function runSourceUpdate(
		action: () => Promise<KubeconfigSourcesSummary>,
		busy = true,
	) {
		if (busy) sourceActionBusy = true;
		try {
			applySources(await action());
		} catch (error) {
			sourceActionError = error;
		} finally {
			if (busy) sourceActionBusy = false;
		}
	}

	function saveEnvVar() {
		void runSourceUpdate(() => setBackendKubeconfigEnvVar(client, envDraft));
	}

	function resetEnvVar() {
		getSettingsSnapshot().resetKubeconfigEnvVar();
		envDraft = DEFAULT_KUBECONFIG_ENV_VAR;
		void runSourceUpdate(() =>
			setBackendKubeconfigEnvVar(client, DEFAULT_KUBECONFIG_ENV_VAR),
		);
	}

	function addPastedPaths() {
		if (!pathDraft.trim()) return;
		void runSourceUpdate(() => addKubeconfigPaths(client, [pathDraft])).then(() => {
			pathDraft = "";
		});
	}

	function movePath(index: number, direction: -1 | 1) {
		if (!sources) return;
		const next = sources.paths.map((entry) => entry.path);
		[next[index], next[index + direction]] = [
			next[index + direction],
			next[index],
		];
		void runSourceUpdate(() => reorderKubeconfigPaths(client, next));
	}
</script>

<FieldGroup>
	<SettingsRow title="Active sources" description={sourceDescription()}>
		<Badge variant="outline">{sources?.sourceLabel ?? DEFAULT_KUBECONFIG_ENV_VAR}</Badge>
	</SettingsRow>

	{#if sourceError || (sources?.warnings.length ?? 0) > 0}
		<div class="flex flex-col gap-2">
			{#if sourceError}
				<FriendlyError
					error={sourceError}
					context={{
						operation: "contextLoad",
						fallbackTitle: "Kubeconfig sources failed",
					}}
				/>
			{/if}
			{#each sources?.warnings ?? [] as warning (`${warning.source}:${warning.path ?? warning.message}`)}
				<Alert>
					<AlertTitle>Kubeconfig source warning</AlertTitle>
					<AlertDescription>
						{warning.path ? `${warning.path}: ` : ""}{warning.message}
					</AlertDescription>
				</Alert>
			{/each}
		</div>
	{/if}

	<SettingsRow
		title="Environment variable"
		description="Variable holding kubeconfig paths. Default is read when unset."
	>
		<div class="flex flex-wrap items-center gap-2">
			<Input
				class="h-8 w-48"
				bind:value={envDraft}
				placeholder={DEFAULT_KUBECONFIG_ENV_VAR}
				aria-label="Kubeconfig env var"
				oninput={() => getSettingsSnapshot().setKubeconfigEnvVar(envDraft)}
			/>
			<Button
				type="button"
				variant="outline"
				size="sm"
				disabled={sourceBusy}
				onclick={saveEnvVar}
			>
				Save
			</Button>
			<Button
				type="button"
				variant="ghost"
				size="icon-sm"
				disabled={sourceBusy}
				aria-label="Reset kubeconfig env var"
				onclick={resetEnvVar}
			>
				<RotateCcw />
			</Button>
		</div>
	</SettingsRow>

	<SettingsRow
		title="Show source labels"
		description="Marks contexts with kubeconfig source they were discovered from."
	>
		<Switch
			checked={settings.showKubeconfigSourceLabels}
			onCheckedChange={(show) =>
				void runSourceUpdate(() => setShowKubeconfigSourceLabels(client, show))}
			aria-label="Show kubeconfig source labels"
		/>
	</SettingsRow>

	<div class="rounded-md border bg-card p-3">
		<div class="flex flex-wrap items-start justify-between gap-3">
			<div class="min-w-0 space-y-1">
				<div class="text-sm font-medium">Added kubeconfig paths</div>
				<p class="text-xs text-muted-foreground">
					Extra kubeconfig files KubeCove reads beside environment variable.
				</p>
			</div>
			<Button
				type="button"
				variant="outline"
				size="sm"
				disabled={sourceBusy}
				onclick={() => void runSourceUpdate(() => pickKubeconfigPaths(client))}
			>
				<FilePlus data-icon="inline-start" />
				Choose files
			</Button>
		</div>

		<div class="mt-3 flex flex-col gap-2">
			{#if sources?.paths.length}
				{#each sources.paths as entry, index (entry.path)}
					<div class="flex items-center gap-2 rounded-md border bg-background p-2">
						<div class="min-w-0 flex-1">
							<div class="truncate text-xs font-medium">{pathFileName(entry.path)}</div>
							<div class="truncate text-xs text-muted-foreground">{entry.path}</div>
						</div>
						<Button
							type="button"
							variant="ghost"
							size="icon-sm"
							disabled={sourceBusy || index === 0}
							aria-label="Move kubeconfig path up"
							onclick={() => movePath(index, -1)}
						>
							<ArrowUp />
						</Button>
						<Button
							type="button"
							variant="ghost"
							size="icon-sm"
							disabled={sourceBusy || index === sources.paths.length - 1}
							aria-label="Move kubeconfig path down"
							onclick={() => movePath(index, 1)}
						>
							<ArrowDown />
						</Button>
						<Button
							type="button"
							variant="ghost"
							size="icon-sm"
							disabled={sourceBusy}
							aria-label="Remove kubeconfig path"
							onclick={() =>
								void runSourceUpdate(() =>
									removeKubeconfigPath(client, entry.path),
								)}
						>
							<Trash2 />
						</Button>
					</div>
				{/each}
			{:else}
				<div class="rounded-md border bg-background p-3 text-xs text-muted-foreground">
					No app-added kubeconfig paths.
				</div>
			{/if}

			<Textarea
				bind:value={pathDraft}
				placeholder="/path/to/kubeconfig.yaml"
				aria-label="Paste kubeconfig paths"
				rows={3}
			/>
			<div class="flex justify-end">
				<Button
					type="button"
					variant="outline"
					size="sm"
					disabled={sourceBusy || !pathDraft.trim()}
					onclick={addPastedPaths}
				>
					<Plus data-icon="inline-start" />
					Add pasted paths
				</Button>
			</div>
		</div>
	</div>
</FieldGroup>
