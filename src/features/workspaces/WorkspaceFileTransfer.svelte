<script lang="ts">
	import type { Snippet } from "svelte";
	import { Download, Upload } from "lucide-svelte";
	import { Badge, Button, Card, CardContent, CardDescription, CardHeader, CardTitle, SegmentedControl } from "@/components/ui/svelte";
	import { USE_BROWSER_DEV_MOCKS } from "@/lib/build-env";
	import { pickWorkspaceImportJson, saveWorkspaceExportJson, type TauriClient } from "@/lib/tauri";
	import type { SavedWorkspace } from "@/lib/workspace-model";
	import { buildWorkspaceImportPreview, serializeWorkspaceExport, slugifyWorkspaceName, type WorkspaceImportAction, type WorkspaceImportPreview } from "./workspace-sharing";
	import { workspaceStore } from "./workspaceStore";

	let { client, sortedWorkspaces, children }: {
		client: TauriClient;
		sortedWorkspaces: SavedWorkspace[];
		children: Snippet<[(workspaces: SavedWorkspace[]) => Promise<void>]>;
	} = $props();
	const workspaces = workspaceStore.workspaces;
	let importFileInput = $state<HTMLInputElement | null>(null);
	let workspaceTransferMessage = $state("");
	let workspaceTransferError = $state("");
	let importPreview = $state.raw<WorkspaceImportPreview | null>(null);
	let importActions = $state<Record<string, WorkspaceImportAction>>({});
	// Raw text and the workspace each matched item would replace, captured at preview time so apply can detect changes.
	let importRaw = "";
	let previewTargets = new Map<string, SavedWorkspace>();
	// Bumped when a picker starts, so a read that started under an older picker is dropped.
	let importGeneration = 0;

	const newImportOptions: { value: WorkspaceImportAction; label: string }[] = [
		{ value: "add", label: "Add" },
		{ value: "skip", label: "Skip" },
	];
	const collisionImportOptions: { value: WorkspaceImportAction; label: string }[] = [
		{ value: "add", label: "Add copy" },
		{ value: "replace", label: "Replace" },
		{ value: "skip", label: "Skip" },
	];

	const selectedImportCount = $derived(
		importPreview
			? importPreview.items.filter(
					(item) => (importActions[item.id] ?? item.defaultAction) !== "skip",
				).length
			: 0,
	);
	function workspaceFileName(workspaces: SavedWorkspace[]): string {
		if (workspaces.length === 1) {
			return `${slugifyWorkspaceName(workspaces[0].sharedKey ?? workspaces[0].name)}.kubecove-workspace.json`;
		}
		return "kubecove-workspaces.json";
	}

	function errorText(cause: unknown): string {
		return cause instanceof Error ? cause.message : String(cause);
	}

	async function exportWorkspaces(workspaces: SavedWorkspace[]) {
		if (workspaces.length === 0) return;
		workspaceTransferError = "";
		workspaceTransferMessage = "";
		try {
			const fileName = workspaceFileName(workspaces);
			const content = serializeWorkspaceExport(workspaces);
			let saved: boolean;
			if (!USE_BROWSER_DEV_MOCKS) {
				const url = URL.createObjectURL(
					new Blob([content], { type: "application/json" }),
				);
				const link = document.createElement("a");
				link.href = url;
				link.download = fileName;
				document.body.append(link);
				link.click();
				link.remove();
				window.setTimeout(() => URL.revokeObjectURL(url), 0);
				saved = true;
			} else {
				saved = await saveWorkspaceExportJson(client, fileName, content);
			}
			workspaceTransferMessage = saved
				? `Exported ${workspaces.length === 1 ? workspaces[0].name : `${workspaces.length} workspaces`}.`
				: "Export canceled.";
		} catch (error) {
			workspaceTransferError = errorText(error);
		}
	}

	// Starting either picker discards the previous preview and decisions, and returns the generation its reads must match.
	function beginImportPick(): number {
		importGeneration += 1;
		clearImportPreview();
		workspaceTransferError = "";
		workspaceTransferMessage = "";
		return importGeneration;
	}

	function clearImportPreview() {
		importPreview = null;
		importActions = {};
		importRaw = "";
		previewTargets = new Map();
	}

	async function importWorkspaces() {
		const generation = beginImportPick();
		try {
			const raw = await pickWorkspaceImportJson(client);
			if (generation !== importGeneration) return;
			if (!raw) {
				workspaceTransferMessage = "Import canceled.";
				return;
			}
			showImportPreview(raw);
		} catch (error) {
			if (generation === importGeneration) workspaceTransferError = errorText(error);
		}
	}

	function showImportPreview(raw: string) {
		const preview = buildWorkspaceImportPreview(raw, $workspaces);
		importRaw = raw;
		previewTargets = new Map(
			preview.items.flatMap((item) => {
				const target = $workspaces.find((workspace) => workspace.id === item.existingWorkspaceId);
				return target ? [[item.id, target] as const] : [];
			}),
		);
		importPreview = preview;
		importActions = Object.fromEntries(
			preview.items.map((item) => [item.id, item.defaultAction]),
		);
		workspaceTransferMessage = `Loaded ${preview.items.length} workspace ${preview.items.length === 1 ? "item" : "items"}.`;
	}

	function openBrowserImportPicker() {
		beginImportPick();
		importFileInput?.click();
	}

	async function readBrowserImportFile(event: Event) {
		const input = event.currentTarget;
		if (!(input instanceof HTMLInputElement)) return;
		const generation = importGeneration;
		const file = input.files?.[0];
		input.value = "";
		if (!file) return;
		workspaceTransferError = "";
		workspaceTransferMessage = "";
		if (file.size > 2 * 1024 * 1024) {
			workspaceTransferError = "Workspace import files must be 2 MiB or smaller.";
			return;
		}
		try {
			const raw = await file.text();
			if (generation === importGeneration) showImportPreview(raw);
		} catch (error) {
			if (generation === importGeneration) workspaceTransferError = errorText(error);
		}
	}

	function setImportAction(itemId: string, action: WorkspaceImportAction) {
		importActions = { ...importActions, [itemId]: action };
	}

	function applyImportPreview() {
		const preview = importPreview;
		if (!preview) return;
		// A replace overwrites the workspace matched at preview time. If that workspace is gone or changed, the
		// preview was built from a store that no longer exists, so show it again for review instead of committing.
		const current = $workspaces;
		const replaceTargetChanged = preview.items.some((item) => {
			if ((importActions[item.id] ?? item.defaultAction) !== "replace") return false;
			const target = current.find((workspace) => workspace.id === item.existingWorkspaceId);
			return target === undefined || target !== previewTargets.get(item.id);
		});
		if (replaceTargetChanged) {
			showImportPreview(importRaw);
			workspaceTransferMessage = "Saved workspaces changed since this preview. Review the import again.";
			return;
		}
		const result = workspaceStore.importWorkspaces(preview, importActions);
		clearImportPreview();
		workspaceTransferError = "";
		workspaceTransferMessage = `Imported ${result.added} new, replaced ${result.replaced}, skipped ${result.skipped}.`;
	}

</script>

<section class="min-w-0">
	<div class="mb-3 flex flex-wrap items-center justify-between gap-2">
		<div class="flex items-baseline gap-2">
			<h1 class="font-heading text-base font-semibold">Workspaces</h1>
			<span class="text-xs text-muted-foreground">
				{sortedWorkspaces.length === 1
					? "1 saved"
					: `${sortedWorkspaces.length} saved`}
			</span>
		</div>
		<div class="flex flex-wrap justify-end gap-2" role="group" aria-label="Workspace sharing">
			{#if !USE_BROWSER_DEV_MOCKS}
				<input
					bind:this={importFileInput}
					type="file"
					accept=".json,application/json"
					class="sr-only"
					aria-label="Choose workspace JSON file"
					onchange={readBrowserImportFile}
				/>
				<Button type="button" variant="outline" size="sm" onclick={openBrowserImportPicker}>
					<Upload data-icon="inline-start" /> Import
				</Button>
			{:else}
				<Button type="button" variant="outline" size="sm" onclick={importWorkspaces}>
					<Upload data-icon="inline-start" /> Import
				</Button>
			{/if}
			<Button
				type="button"
				variant="outline"
				size="sm"
				disabled={sortedWorkspaces.length === 0}
				onclick={() => exportWorkspaces(sortedWorkspaces)}
			>
				<Download data-icon="inline-start" /> Export all
			</Button>
		</div>
	</div>

	{#if workspaceTransferError}
		<div class="mb-3 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
			{workspaceTransferError}
		</div>
	{:else if workspaceTransferMessage}
		<div class="mb-3 rounded-md border bg-surface-1 px-3 py-2 text-xs text-muted-foreground">
			{workspaceTransferMessage}
		</div>
	{/if}

	{#if importPreview}
		<Card size="sm" class="mb-3">
			<CardHeader>
				<CardTitle>Import preview</CardTitle>
				<CardDescription>
					Choose how each shared workspace should land locally.
				</CardDescription>
			</CardHeader>
			<CardContent class="grid gap-3">
				{#each importPreview.items as item (item.id)}
					<div class="grid gap-2 rounded-md border bg-background/40 p-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
						<div class="min-w-0">
							<div class="flex min-w-0 flex-wrap items-center gap-2">
								<span class="truncate text-sm font-medium">{item.workspace.name}</span>
								<Badge variant="outline" class="rounded-sm">{item.workspace.sharedKey}</Badge>
								{#if item.existingWorkspaceName}
									<Badge variant="secondary" class="rounded-sm">
										Matches {item.existingWorkspaceName}
									</Badge>
								{/if}
							</div>
							<div class="mt-1 text-xs text-muted-foreground">
								{item.workspace.scope.clusterContext} / {item.workspace.scope.namespaces.length || "All"} namespaces / {item.workspace.portForwards.length} port forwards
							</div>
						</div>
						<SegmentedControl
							value={importActions[item.id] ?? item.defaultAction}
							options={item.existingWorkspaceId ? collisionImportOptions : newImportOptions}
							ariaLabel={`Import action for ${item.workspace.name}`}
							onChange={(action) => setImportAction(item.id, action)}
						/>
					</div>
				{/each}
				<div class="flex justify-end gap-2">
					<Button
						type="button"
						variant="ghost"
						size="sm"
						onclick={clearImportPreview}
					>
						Cancel
					</Button>
					<Button
						type="button"
						size="sm"
						disabled={selectedImportCount === 0}
						onclick={applyImportPreview}
					>
						Apply import
					</Button>
				</div>
			</CardContent>
		</Card>
	{/if}

	{@render children(exportWorkspaces)}
</section>
