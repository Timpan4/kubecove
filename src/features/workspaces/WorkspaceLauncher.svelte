<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import { FolderOpen, Pencil, Plus, Trash2, X } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Badge,
		Button,
		Card,
		CardAction,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle,
		Checkbox,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyMedia,
		EmptyTitle,
		Field,
		FieldGroup,
		FieldLabel,
		FieldLegend,
		FieldSet,
		Input,
		ScrollArea,
		Select,
		SelectContent,
		SelectGroup,
		SelectItem,
		SelectTrigger,
		SelectValue,
	} from "@/components/ui/svelte";
	import { cnfast } from "@/lib/utils";
	import {
		summarizeWorkspaceScope,
		workspaceScopeContexts,
		type SavedWorkspace,
	} from "@/lib/workspace-model";
	import {
		createTauriClient,
		getKubeconfigSources,
		listKubeContexts,
		listNamespaces,
	} from "@/lib/tauri";
	import { queryKeys } from "@/lib/queryKeys";
	import type { ClusterContext, NamespaceSummary } from "@/lib/types";
	import {
		buildWorkspaceInput,
		pickEffectiveContext,
		uniqueWorkspaceContexts,
	} from "./workspaceLauncherModel";
	import { workspaceStore as workspaceStore } from "./workspaceStore";

	const workspaces = workspaceStore.workspaces;

	// Form state — Svelte 5 runes.
	let editingId = $state<string | null>(null);
	let name = $state("");
	let selectedContext = $state("");
	let selectedGroupContexts = $state<string[]>([]);
	let selectedNamespaces = $state<string[]>([]);

	const client = createTauriClient();
	const sourceQuery = createQuery(() => ({
		queryKey: ["kubeconfig-sources"] as const,
		queryFn: () => getKubeconfigSources(client),
		staleTime: 60_000,
	}));
	const sourceReady = $derived(sourceQuery.isSuccess || sourceQuery.isError);
	const kubeconfigSourceKey = $derived(sourceQuery.data?.sourceKey);
	const contextsQuery = createQuery<ClusterContext[]>(() => ({
		queryKey: queryKeys.kubeContexts(kubeconfigSourceKey),
		queryFn: () => listKubeContexts(client, kubeconfigSourceKey),
		enabled: sourceReady,
		staleTime: 30_000,
	}));
	const contexts = $derived(contextsQuery.data ?? []);
	const contextsPending = $derived(!sourceReady || contextsQuery.isPending);
	const contextsError = $derived(
		sourceQuery.isError
			? sourceQuery.error
			: contextsQuery.isError
				? contextsQuery.error
				: null,
	);

	const sortedWorkspaces = $derived(
		[...$workspaces].sort((left, right) =>
			right.updatedAt.localeCompare(left.updatedAt),
		),
	);
	const editingWorkspace = $derived(
		$workspaces.find((workspace) => workspace.id === editingId) ?? null,
	);
	const effectiveContext = $derived(pickEffectiveContext(selectedContext, contexts));
	const selectedClusterContexts = $derived(
		uniqueWorkspaceContexts(effectiveContext, selectedGroupContexts),
	);
	const selectedContextMissing = $derived(
		effectiveContext.length > 0 &&
			!contexts.some((context) => context.name === effectiveContext),
	);
	const canCreate = $derived(effectiveContext.length > 0 && !selectedContextMissing);
	const namespacesQuery = createQuery<NamespaceSummary[]>(() => ({
		queryKey: queryKeys.namespaces(effectiveContext, kubeconfigSourceKey),
		queryFn: () => listNamespaces(client, effectiveContext, kubeconfigSourceKey),
		enabled: effectiveContext.length > 0 && sourceReady,
		staleTime: 30_000,
	}));
	const namespaces = $derived(namespacesQuery.data ?? []);
	const namespacesPending = $derived(
		effectiveContext.length > 0 && (!sourceReady || namespacesQuery.isPending),
	);
	const namespacesError = $derived(
		namespacesQuery.isError ? namespacesQuery.error : null,
	);

	$effect(() => {
		if (!selectedContext && contexts.length > 0) {
			selectedContext = pickEffectiveContext("", contexts);
		}
	});

	function elementId(prefix: string, value: string): string {
		return `${prefix}-${value.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
	}

	function retryContexts() {
		void sourceQuery.refetch();
		void contextsQuery.refetch();
	}

	function retryNamespaces() {
		void namespacesQuery.refetch();
	}

	function resetForm() {
		editingId = null;
		name = "";
		selectedContext = "";
		selectedGroupContexts = [];
		selectedNamespaces = [];
	}

	function handleContextChange(value: string) {
		selectedContext = value;
		selectedNamespaces = [];
		selectedGroupContexts = selectedGroupContexts.filter(
			(context) => context !== value,
		);
	}

	function toggleNamespace(namespace: string) {
		selectedNamespaces = selectedNamespaces.includes(namespace)
			? selectedNamespaces.filter((item) => item !== namespace)
			: [...selectedNamespaces, namespace];
	}

	function toggleGroupContext(context: string) {
		if (context === effectiveContext) return;
		selectedGroupContexts = selectedGroupContexts.includes(context)
			? selectedGroupContexts.filter((item) => item !== context)
			: [...selectedGroupContexts, context];
	}

	function editWorkspace(workspace: SavedWorkspace) {
		editingId = workspace.id;
		name = workspace.name;
		selectedContext = workspace.scope.clusterContext;
		selectedGroupContexts = workspaceScopeContexts(workspace.scope).filter(
			(context) => context !== workspace.scope.clusterContext,
		);
		selectedNamespaces = workspace.scope.namespaces;
	}

	function handleSubmit(event: SubmitEvent) {
		event.preventDefault();
		if (!canCreate) return;
		const input = buildWorkspaceInput({
			name,
			effectiveContext,
			selectedClusterContexts,
			selectedNamespaces,
			editingWorkspace,
		});
		if (editingWorkspace) {
			workspaceStore.updateWorkspace(editingWorkspace.id, input);
			resetForm();
			return;
		}
		workspaceStore.createWorkspace(input);
		resetForm();
	}

	function deleteWorkspace(workspace: SavedWorkspace) {
		workspaceStore.deleteWorkspace(workspace.id);
		if (editingId === workspace.id) resetForm();
	}

	function workspaceGroupCount(workspace: SavedWorkspace): number {
		return workspaceScopeContexts(workspace.scope).length;
	}
</script>

<section class="flex min-h-full flex-col justify-center p-4 md:p-6">
	<div class="mx-auto grid w-full max-w-6xl gap-4 lg:grid-cols-[minmax(0,1fr)_360px]">
		<section class="min-w-0">
			<div class="mb-3 flex items-baseline gap-2">
				<h1 class="font-heading text-base font-semibold">Workspaces</h1>
				<span class="text-xs text-muted-foreground">
					{sortedWorkspaces.length === 1
						? "1 saved"
						: `${sortedWorkspaces.length} saved`}
				</span>
			</div>

			<div class="grid gap-2">
				{#if contextsPending}
					<Card size="sm">
						<CardContent class="text-xs text-muted-foreground">
							Loading contexts...
						</CardContent>
					</Card>
				{/if}

				{#if contextsError}
					<div class="grid gap-2">
						<FriendlyError
							error={contextsError}
							context={{ operation: "contextLoad", fallbackTitle: "Failed to load contexts" }}
						/>
						<Button
							type="button"
							variant="outline"
							size="sm"
							class="mt-2 w-fit"
							onclick={retryContexts}
						>
							Retry
						</Button>
					</div>
				{/if}

				{#if sortedWorkspaces.length === 0 && !contextsPending}
					<!-- First-run empty state: welcoming, not a bare box. -->
					<Empty class="min-h-52 border border-dashed bg-surface-1/50">
						<EmptyHeader>
							<EmptyMedia variant="icon"><Plus /></EmptyMedia>
							<EmptyTitle>No saved workspaces yet</EmptyTitle>
							<EmptyDescription>
								Create a workspace from a cluster context to start inspecting
								resources, topology, and live sessions.
							</EmptyDescription>
						</EmptyHeader>
					</Empty>
				{/if}

				{#each sortedWorkspaces as workspace (workspace.id)}
					{@const unavailable =
						contexts.length > 0 &&
						!contexts.some(
							(context) => context.name === workspace.scope.clusterContext,
						)}
					<Card
						size="sm"
						elevation="raised"
						class={cnfast(
							"grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]",
							unavailable && "border-amber-500/40 hover:border-amber-500/60",
						)}
					>
						<CardHeader class="min-w-0">
							<div class="flex min-w-0 items-center gap-2">
								<CardTitle class="truncate">{workspace.name}</CardTitle>
								{#if workspaceGroupCount(workspace) > 1}
									<Badge variant="secondary">
										{workspaceGroupCount(workspace)} contexts
									</Badge>
								{/if}
								{#if unavailable}
									<Badge
										variant="outline"
										class="border-amber-500/40 text-amber-300"
									>
										Context unavailable
									</Badge>
								{/if}
							</div>
							<CardDescription class="truncate">
								{summarizeWorkspaceScope(workspace.scope)}
							</CardDescription>
						</CardHeader>
						<CardAction
							class="row-start-auto flex items-center justify-end gap-2 px-3"
							role="group"
							aria-label={`${workspace.name} actions`}
						>
							<Button
								type="button"
								size="sm"
								aria-label={`Open ${workspace.name}`}
								onclick={() => workspaceStore.openWorkspace(workspace.id)}
							>
								<FolderOpen data-icon="inline-start" />
								Open
							</Button>
							<Button
								type="button"
								variant="ghost"
								size="icon-sm"
								aria-label={`Edit ${workspace.name}`}
								onclick={() => editWorkspace(workspace)}
							>
								<Pencil />
							</Button>
							<Button
								type="button"
								variant="ghost"
								size="icon-sm"
								aria-label={`Delete ${workspace.name}`}
								class="text-muted-foreground hover:text-destructive"
								onclick={() => deleteWorkspace(workspace)}
							>
								<Trash2 />
							</Button>
						</CardAction>
					</Card>
				{/each}
			</div>
		</section>

		<Card size="sm" elevation="flat" class="self-start lg:sticky lg:top-2">
			<form class="contents" onsubmit={handleSubmit}>
				<CardHeader>
					<CardTitle>
						{editingWorkspace ? "Edit workspace" : "New workspace"}
					</CardTitle>
					{#if editingWorkspace}
						<CardAction>
							<Button
								type="button"
								variant="ghost"
								size="icon-sm"
								aria-label="Cancel edit"
								onclick={resetForm}
							>
								<X />
							</Button>
						</CardAction>
					{/if}
				</CardHeader>
				<CardContent>
					<FieldGroup class="gap-3">
						<Field>
							<FieldLabel for="workspace-name">Name</FieldLabel>
							<Input
								id="workspace-name"
								bind:value={name}
								placeholder={effectiveContext || "Workspace name"}
							/>
						</Field>

						<Field>
							<FieldLabel>Context</FieldLabel>
							<Select
								value={effectiveContext}
								items={contexts.map((context) => ({
									value: context.name,
									label: context.name,
								}))}
								onValueChange={handleContextChange}
							>
								<SelectTrigger class="w-full">
									<SelectValue
										placeholder={contextsPending
											? "Loading contexts"
											: "Select context"}
									/>
								</SelectTrigger>
								<SelectContent>
									<SelectGroup>
										{#each contexts as context (context.name)}
											<SelectItem value={context.name}>{context.name}</SelectItem>
										{/each}
									</SelectGroup>
								</SelectContent>
							</Select>
						</Field>

						{#if contexts.length > 1}
							<FieldSet class="gap-1.5">
								<FieldLegend variant="label" class="text-muted-foreground">
									Cluster group
								</FieldLegend>
								<ScrollArea class="h-32 rounded-md border bg-background/40">
									<div class="p-1">
										{#each contexts as context (context.name)}
											{@const checkboxId = elementId(
												"workspace-context",
												context.name,
											)}
											{@const checked =
												context.name === effectiveContext ||
												selectedClusterContexts.includes(context.name)}
											<Field
												orientation="horizontal"
												class="h-7 items-center gap-2 rounded-sm px-2 text-xs hover:bg-muted"
											>
												<Checkbox
													id={checkboxId}
													{checked}
													disabled={context.name === effectiveContext}
													onCheckedChange={() => toggleGroupContext(context.name)}
												/>
												<FieldLabel
													for={checkboxId}
													class="min-w-0 flex-1 cursor-pointer truncate font-normal"
												>
													{context.name}
												</FieldLabel>
											</Field>
										{/each}
									</div>
								</ScrollArea>
							</FieldSet>
						{/if}

						<FieldSet class="gap-1.5">
							<FieldLegend variant="label" class="text-muted-foreground">
								Namespaces
							</FieldLegend>
							<ScrollArea class="h-52 rounded-md border bg-background/40">
								<div class="p-1">
									{#if namespacesPending && effectiveContext}
										<div class="px-2 py-1.5 text-xs text-muted-foreground">
											Loading namespaces...
										</div>
							{/if}
							{#if namespacesError}
								<div class="grid gap-2 px-2 py-1.5">
									<FriendlyError
										mode="compact"
										error={namespacesError}
										context={{
											operation: "resourcesLoad",
											fallbackTitle: "Failed to load namespaces",
											partial: true,
										}}
									/>
									<Button
										type="button"
												variant="outline"
												size="sm"
												class="w-fit"
									onclick={retryNamespaces}
											>
												Retry
											</Button>
										</div>
									{/if}
									{#if !namespacesPending && !namespacesError && namespaces.length === 0}
										<div class="px-2 py-1.5 text-xs text-muted-foreground">
											All namespaces
										</div>
									{/if}
									{#if !namespacesError}
										{#each namespaces as namespace (namespace.name)}
											{@const checkboxId = elementId(
												"workspace-namespace",
												namespace.name,
											)}
											<Field
												orientation="horizontal"
												class="h-7 items-center gap-2 rounded-sm px-2 text-xs hover:bg-muted"
											>
												<Checkbox
													id={checkboxId}
													checked={selectedNamespaces.includes(namespace.name)}
													onCheckedChange={() => toggleNamespace(namespace.name)}
												/>
												<FieldLabel
													for={checkboxId}
													class="min-w-0 flex-1 cursor-pointer truncate font-normal"
												>
													{namespace.name}
												</FieldLabel>
											</Field>
										{/each}
									{/if}
								</div>
							</ScrollArea>
						</FieldSet>

						<Button type="submit" size="lg" disabled={!canCreate}>
							{#if editingWorkspace}
								<FolderOpen data-icon="inline-start" />
							{:else}
								<Plus data-icon="inline-start" />
							{/if}
							{editingWorkspace ? "Save workspace" : "Create workspace"}
						</Button>
					</FieldGroup>
				</CardContent>
			</form>
		</Card>
	</div>
</section>
