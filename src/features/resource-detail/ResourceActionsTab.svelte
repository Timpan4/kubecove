<script lang="ts">
	import { useQueryClient } from "@tanstack/svelte-query";
	import { untrack } from "svelte";
	import {
		Button,
		Dialog,
		DialogContent,
		DialogDescription,
		DialogHeader,
		DialogTitle,
		Input,
		Spinner,
	} from "@/components/ui/svelte";
	import { LockKeyhole, Search } from "lucide-svelte";
	import type { TauriClient } from "@/lib/tauri";
	import type { ResourceSummary } from "@/lib/types";
	import { getSettingsSnapshot } from "@/lib/settings-store";
	import { buildCompactUnifiedDiff, diffLineClassName } from "./yamlTabDiff";
	import {
		actionAllowed,
		actionForRow,
		currentRowLabel,
		fallbackPermission,
		permissionForAction,
		resourceActionRows,
		type ResourceActionRowId,
	} from "./resource-actions-model";
	import {
		addEnvironment,
		addMetadata,
		addQuantity,
		clearDrafts,
		containerDraft,
		environmentRows,
		hasPendingEdits,
		metadataRows,
		type MetadataField,
		pendingEditCount,
		pendingRowLabel,
		type QuantityField,
		quantityRows,
		removeEnvironment,
		removeMetadata,
		removeQuantity,
		renameEnvironment,
		restoreEnvironment,
		restoreQuantity,
		revertRow,
		setEnvironment,
		setImage,
		setMetadata,
		setQuantity,
		setReplicas,
	} from "./resourceActionDrafts";
	import { createResourceActionSession } from "./resourceActionSession";

	type NewEntry = { name: string; value: string };
	const QUANTITY_FIELDS: QuantityField[] = ["requests", "limits"];

	let {
		client,
		resource,
		kubeconfigSourceKey,
	}: {
		client: TauriClient;
		resource: ResourceSummary;
		kubeconfigSourceKey?: string;
	} = $props();

	let filter = $state("");
	let expanded = $state<ResourceActionRowId | null>(null);
	let selectedRevision = $state<number | null>(null);
	let newEnvironment = $state<Record<string, NewEntry>>({});
	let newQuantities = $state<Record<string, Partial<Record<QuantityField, NewEntry>>>>({});
	let newMetadata = $state<Record<MetadataField, NewEntry>>({ labels: { name: "", value: "" }, annotations: { name: "", value: "" } });

	const queryClient = useQueryClient();
	const session = untrack(() =>
		createResourceActionSession({
			client,
			queryClient,
			skipReview: () => getSettingsSnapshot().skipChangeReview,
			onDraftsApplied: () => {
				newEnvironment = {};
				newQuantities = {};
			},
		}),
	);
	session.retarget(untrack(() => ({ resource, kubeconfigSourceKey })));
	let current = $state.raw(session.current);
	$effect.pre(() => {
		if (!session.retarget({ resource, kubeconfigSourceKey })) return;
		filter = "";
		expanded = null;
		selectedRevision = null;
		newEnvironment = {};
		newQuantities = {};
		newMetadata = { labels: { name: "", value: "" }, annotations: { name: "", value: "" } };
	});
	$effect(() => session.subscribe((next) => (current = next)));

	const actionState = $derived(current.actionState);
	const edits = $derived(current.edits);
	const review = $derived(current.review);
	const rows = $derived(resourceActionRows(resource));
	const visibleRows = $derived.by(() => {
		const query = filter.trim().toLocaleLowerCase();
		return rows.filter((row) => !query || `${row.label} ${row.group} ${row.search}`.toLocaleLowerCase().includes(query));
	});
	const groups = $derived([...new Set(visibleRows.map((row) => row.group))]);
	const pending = $derived(hasPendingEdits(edits));
	const diffLines = $derived(review?.preview ? buildCompactUnifiedDiff(review.preview.beforeYaml, review.preview.afterYaml) : []);
	const reviewBusy = $derived(review?.status === "checking" || review?.status === "executing");

	function revert(row: ResourceActionRowId) {
		session.draft(revertRow(row));
		if (row === "environment") newEnvironment = {};
		if (row === "resources") newQuantities = {};
	}

	function addEnvironmentEntry(container: string) {
		const entry = newEnvironment[container] ?? { name: "", value: "" };
		if (session.draft(addEnvironment(container, entry.name, entry.value))) newEnvironment = { ...newEnvironment, [container]: { name: "", value: "" } };
	}

	function addQuantityEntry(container: string, field: QuantityField) {
		const entry = newQuantities[container]?.[field] ?? { name: "", value: "" };
		if (session.draft(addQuantity(container, field, entry.name, entry.value))) newQuantities = { ...newQuantities, [container]: { ...newQuantities[container], [field]: { name: "", value: "" } } };
	}

	function addMetadataEntry(field: MetadataField) {
		const entry = newMetadata[field];
		if (session.draft(addMetadata(field, entry.name, entry.value))) newMetadata = { ...newMetadata, [field]: { name: "", value: "" } };
	}
</script>

<div class="flex h-full min-h-0 flex-col overflow-auto text-sm">
	<div class="flex flex-wrap items-center justify-between gap-3 border-b pb-3">
		<div class="flex flex-wrap items-center gap-2 text-xs">
			{#if actionState?.replicas !== undefined}<span class="rounded-full border bg-muted/30 px-2 py-1 tabular-nums">Desired {actionState.replicas}</span>{/if}
			{#if actionState?.paused !== undefined}<span class="rounded-full border bg-muted/30 px-2 py-1">{actionState.paused ? "Paused" : "Running"}</span>{/if}
			{#if actionState?.revisions.length}<span class="rounded-full border bg-muted/30 px-2 py-1">{actionState.revisions.length} revisions</span>{/if}
			{#if current.loading}<span class="inline-flex items-center gap-2 text-muted-foreground"><Spinner />Loading action permissions</span>{/if}
		</div>
		<label class="flex h-8 w-56 items-center gap-2 rounded-md border bg-muted/20 px-2 text-muted-foreground focus-within:ring-2 focus-within:ring-ring">
			<Search class="size-3.5 shrink-0" aria-hidden="true" />
			<Input class="h-full border-0 bg-transparent p-0 shadow-none focus-visible:ring-0" aria-label="Resource action filter" placeholder="Filter actions" bind:value={filter} />
			<kbd class="text-[0.65rem]">/</kbd>
		</label>
	</div>

	{#if current.loadError}<div class="flex flex-wrap items-center justify-between gap-2 border-b border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive" role="status"><span>Unable to load action state and permissions. Actions remain locked. {current.loadError}</span><Button variant="ghost" size="sm" disabled={current.loading} onclick={() => session.retry()}>Retry permission check</Button></div>{/if}
	{#if current.result}<p class="border-b border-emerald-500/30 bg-emerald-500/5 px-3 py-2 text-xs text-emerald-300" role="status">{current.result}</p>{/if}
	{#if current.inlineStatus}<p class="flex items-center gap-2 border-b px-3 py-2 text-xs text-muted-foreground" role="status"><Spinner />{current.inlineStatus}</p>{/if}
	{#if rows.length === 0}<p class="py-8 text-muted-foreground">Actions are not available for this resource.</p>{/if}
	{#if rows.length > 0 && visibleRows.length === 0}<p class="py-8 text-muted-foreground">No action matches “{filter}”.</p>{/if}

	{#each groups as group (group)}
		{#if group}<h3 class="mt-4 border-b px-1 pb-2 text-xs font-semibold text-muted-foreground">{group}</h3>{/if}
		<div class="divide-y">
			{#each visibleRows.filter((row) => row.group === group) as row (row.id)}
				{@const rowAction = actionForRow(row, actionState)}
				{@const permission = permissionForAction(actionState, rowAction)}
				{@const canAct = permission?.allowed === true}
				{@const isExpanded = expanded === row.id}
				{@const pendingText = pendingRowLabel(edits, row.id)}
				<article class="py-3 pl-1" data-resource-action={row.id}>
					<div class="flex flex-wrap items-start justify-between gap-3">
						<div class="min-w-0 flex-1">
							<div class="flex items-center gap-2 font-medium">{row.label}{#if pendingText}<span class="rounded-full bg-sky-500/10 px-2 py-0.5 text-[0.7rem] font-normal text-sky-300">Pending</span>{/if}</div>
							<p class="mt-1 break-words text-xs text-muted-foreground">{currentRowLabel(row, actionState, resource.kind)}</p>
							{#if pendingText}<p class="mt-1 text-xs text-sky-300">{pendingText}</p>{/if}
							{#if !canAct}<p class="mt-1 flex items-center gap-1 text-xs text-muted-foreground"><LockKeyhole class="size-3.5" aria-hidden="true" />Needs {permission?.requiredPermission || fallbackPermission(rowAction, resource.kind)}{#if permission?.reason}<span>· {permission.reason}</span>{:else if !permission}<span>· Permission has not been confirmed</span>{/if}</p>{/if}
						</div>
						<div class="flex shrink-0 items-center gap-2">
							{#if pendingText && row.action === "edit"}<Button variant="ghost" size="sm" onclick={() => revert(row.id)}>Revert</Button>{/if}
							{#if row.id === "delete"}
								<Button variant="destructive" size="sm" disabled={!canAct || current.loading} onclick={() => session.review("delete")}>Delete</Button>
							{:else if row.id === "restart"}
								<Button variant="outline" size="sm" disabled={!canAct || current.loading} onclick={() => session.review("restart")}>Restart</Button>
							{:else if row.id === "rollout"}
								<Button variant="outline" size="sm" disabled={!canAct || current.loading} onclick={() => session.review(rowAction)}>{actionState?.paused ? "Resume" : "Pause"}</Button>
							{:else if row.id === "rollback"}
								<Button variant="outline" size="sm" disabled={!canAct || current.loading || actionState?.revisions.length === 0} onclick={() => (expanded = isExpanded ? null : row.id)}>{isExpanded ? "Close" : "Roll back"}</Button>
							{:else}
								<Button variant="outline" size="sm" disabled={!canAct || current.loading} aria-expanded={isExpanded} onclick={() => (expanded = isExpanded ? null : row.id)}>{isExpanded ? "Done" : "Edit"}</Button>
							{/if}
						</div>
					</div>

					{#if isExpanded && canAct && row.id === "replicas"}
						<div class="mt-3 max-w-56"><label class="grid gap-1 text-xs text-muted-foreground">Desired replicas<Input type="number" min="0" step="1" aria-label="Desired replicas" value={String(edits.replicas ?? actionState?.replicas ?? 0)} oninput={(event: Event & { currentTarget: HTMLInputElement }) => session.draft(setReplicas(Number(event.currentTarget.value)))} /></label></div>
					{/if}
					{#if isExpanded && canAct && row.id === "rollback"}
						<div class="mt-3 space-y-2">
							{#if !actionState?.revisions.length}<p class="text-xs text-muted-foreground">No owned revisions are available.</p>{/if}
							{#each actionState?.revisions ?? [] as revision (revision.revision)}
								<label class="flex flex-wrap items-center gap-2 rounded-md border px-3 py-2 text-xs">
									<input type="radio" name="resource-action-revision" value={revision.revision} checked={selectedRevision === revision.revision} onchange={() => (selectedRevision = revision.revision)} />
									<span class="font-medium">Revision {revision.revision}</span><span class="min-w-0 flex-1 truncate font-mono text-muted-foreground">{revision.images.join(", ")}</span>
								</label>
							{/each}
							<Button size="sm" disabled={selectedRevision === null} onclick={() => selectedRevision !== null && session.review("rollback", selectedRevision)}>Review rollback</Button>
						</div>
					{/if}
					{#if isExpanded && canAct && ["image", "environment", "resources"].includes(row.id) && actionState}
						<div class="mt-3 space-y-4 border-l pl-3">
							{#each actionState.containers as container (container.name)}
								<section class="space-y-2">
									<h4 class="text-xs font-semibold">{container.name}</h4>
									{#if row.id === "image"}
										<label class="grid max-w-xl gap-1 text-xs text-muted-foreground">Container image<Input aria-label="Container image" value={containerDraft(edits, container.name).image ?? container.image} oninput={(event: Event & { currentTarget: HTMLInputElement }) => session.draft(setImage(container.name, event.currentTarget.value))} /></label>
									{:else if row.id === "environment"}
										<div class="space-y-2">
											{#each environmentRows(edits, actionState, container.name) as variable, index (variable.isNew ? `new:${index}` : variable.name)}
												<div class="grid gap-2 sm:grid-cols-[minmax(8rem,0.7fr)_minmax(10rem,1.3fr)_auto]">
													<Input aria-label={`Environment name ${container.name} ${variable.name}`} value={variable.name} disabled={!variable.isNew} oninput={(event: Event & { currentTarget: HTMLInputElement }) => session.draft(renameEnvironment(container.name, variable.name, event.currentTarget.value))} />
													<Input aria-label={`Environment value ${container.name} ${variable.name}`} type="text" placeholder={variable.referenced ? "Value from Secret or ConfigMap" : "Value"} value={variable.value} disabled={variable.removed} oninput={(event: Event & { currentTarget: HTMLInputElement }) => session.draft(setEnvironment(container.name, variable.name, event.currentTarget.value))} />
													<Button variant="ghost" size="sm" onclick={() => session.draft(variable.removed ? restoreEnvironment(container.name, variable.name) : removeEnvironment(container.name, variable.name))}>{variable.removed ? "Restore" : "Remove"}</Button>
												</div>
											{/each}
											<div class="grid gap-2 sm:grid-cols-[minmax(8rem,0.7fr)_minmax(10rem,1.3fr)_auto]">
												<Input aria-label={`New environment name ${container.name}`} placeholder="Variable name" value={newEnvironment[container.name]?.name ?? ""} oninput={(event: Event & { currentTarget: HTMLInputElement }) => (newEnvironment = { ...newEnvironment, [container.name]: { name: event.currentTarget.value, value: newEnvironment[container.name]?.value ?? "" } })} />
												<Input aria-label={`New environment value ${container.name}`} placeholder="Value" value={newEnvironment[container.name]?.value ?? ""} oninput={(event: Event & { currentTarget: HTMLInputElement }) => (newEnvironment = { ...newEnvironment, [container.name]: { name: newEnvironment[container.name]?.name ?? "", value: event.currentTarget.value } })} />
												<Button variant="outline" size="sm" disabled={!newEnvironment[container.name]?.name.trim()} onclick={() => addEnvironmentEntry(container.name)}>Add</Button>
											</div>
										</div>
									{:else}
										<div class="space-y-3">
											{#each QUANTITY_FIELDS as field (field)}
												<div class="space-y-2">
													<h5 class="text-xs font-medium">{field === "requests" ? "Requests" : "Limits"}</h5>
													{#each quantityRows(edits, actionState, container.name, field) as quantity (quantity.key)}
														<div class="grid gap-2 sm:grid-cols-[minmax(8rem,0.7fr)_minmax(10rem,1.3fr)_auto]">
															<Input aria-label={`${field === "requests" ? "Request" : "Limit"} ${container.name} ${quantity.key}`} value={quantity.key} disabled />
															<Input aria-label={`${field === "requests" ? "Request" : "Limit"} value ${container.name} ${quantity.key}`} value={quantity.value} oninput={(event: Event & { currentTarget: HTMLInputElement }) => session.draft(setQuantity(container.name, field, quantity.key, event.currentTarget.value))} />
															<Button variant="ghost" size="sm" onclick={() => session.draft(quantity.removed ? restoreQuantity(container.name, field, quantity.key) : removeQuantity(container.name, field, quantity.key))}>{quantity.removed ? "Restore" : "Remove"}</Button>
														</div>
													{/each}
													<div class="grid gap-2 sm:grid-cols-[minmax(8rem,0.7fr)_minmax(10rem,1.3fr)_auto]">
														<Input aria-label={`New ${field === "requests" ? "request" : "limit"} name ${container.name}`} placeholder="Resource name" value={newQuantities[container.name]?.[field]?.name ?? ""} oninput={(event: Event & { currentTarget: HTMLInputElement }) => (newQuantities = { ...newQuantities, [container.name]: { ...newQuantities[container.name], [field]: { name: event.currentTarget.value, value: newQuantities[container.name]?.[field]?.value ?? "" } } })} />
														<Input aria-label={`New ${field === "requests" ? "request" : "limit"} value ${container.name}`} placeholder="Quantity" value={newQuantities[container.name]?.[field]?.value ?? ""} oninput={(event: Event & { currentTarget: HTMLInputElement }) => (newQuantities = { ...newQuantities, [container.name]: { ...newQuantities[container.name], [field]: { name: newQuantities[container.name]?.[field]?.name ?? "", value: event.currentTarget.value } } })} />
														<Button variant="outline" size="sm" disabled={!newQuantities[container.name]?.[field]?.name.trim()} onclick={() => addQuantityEntry(container.name, field)}>Add</Button>
													</div>
												</div>
											{/each}
										</div>
									{/if}
								</section>
							{/each}
						</div>
					{/if}
					{#if isExpanded && canAct && (row.id === "labels" || row.id === "annotations") && actionState}
						{@const field = row.id}
						<div class="mt-3 space-y-2 border-l pl-3">
							{#each metadataRows(edits, actionState, field) as entry (entry.key)}
								<div class="grid gap-2 sm:grid-cols-[minmax(8rem,0.7fr)_minmax(10rem,1.3fr)_auto]">
									<Input aria-label={`${row.label} key ${entry.key}`} value={entry.key} disabled />
									<Input aria-label={`${row.label} value ${entry.key}`} value={entry.value} oninput={(event: Event & { currentTarget: HTMLInputElement }) => session.draft(setMetadata(field, entry.key, event.currentTarget.value))} />
									<Button variant="ghost" size="sm" onclick={() => session.draft(removeMetadata(field, entry.key))}>Remove</Button>
								</div>
							{/each}
							<div class="grid gap-2 sm:grid-cols-[minmax(8rem,0.7fr)_minmax(10rem,1.3fr)_auto]">
								<Input aria-label={`New ${row.label.toLocaleLowerCase()} key`} placeholder="Key" value={newMetadata[field].name} oninput={(event: Event & { currentTarget: HTMLInputElement }) => (newMetadata = { ...newMetadata, [field]: { ...newMetadata[field], name: event.currentTarget.value } })} />
								<Input aria-label={`New ${row.label.toLocaleLowerCase()} value`} placeholder="Value" value={newMetadata[field].value} oninput={(event: Event & { currentTarget: HTMLInputElement }) => (newMetadata = { ...newMetadata, [field]: { ...newMetadata[field], value: event.currentTarget.value } })} />
								<Button variant="outline" size="sm" disabled={!newMetadata[field].name.trim()} onclick={() => addMetadataEntry(field)}>Add</Button>
							</div>
						</div>
					{/if}
				</article>
			{/each}
		</div>
	{/each}

	{#if current.loadError && rows.length === 0}<p class="py-6 text-xs text-muted-foreground">The resource kind is outside the selected-resource action allowlist.</p>{/if}

	{#if pending}
		<div class="sticky bottom-0 mt-auto flex flex-wrap items-center justify-between gap-3 border-t bg-background px-3 py-2 shadow-sm">
			<div class="text-xs text-muted-foreground">{pendingEditCount(edits)} pending changes</div>
			<div class="flex items-center gap-2"><Button variant="ghost" size="sm" onclick={() => session.draft(clearDrafts)}>Discard</Button><Button size="sm" disabled={!actionAllowed(actionState, "edit") || current.loading} onclick={() => session.review("edit")}>Check changes</Button></div>
		</div>
	{/if}
</div>

{#if review}
	<Dialog open={review.open} onOpenChange={(open: boolean) => { if (!open) session.closeReview(); }}>
		<DialogContent class="max-h-[min(85vh,52rem)] max-w-4xl overflow-auto">
			<DialogHeader>
				<DialogTitle>Review {review.action === "delete" ? `deletion of ${review.name}` : `${review.action} for ${review.name}`}</DialogTitle>
				<DialogDescription>{review.preview?.effect ?? (review.status === "executing" ? "Applying the reviewed action…" : review.status === "checking" ? "Checking this exact resource with the API server…" : "The API server could not validate this action.")}</DialogDescription>
			</DialogHeader>
			<div class="flex justify-end"><Button variant="ghost" size="sm" disabled={review.status === "executing"} aria-label="Close review" onclick={() => session.closeReview()}>{reviewBusy ? "Cancel check" : "Close review"}</Button></div>
			{#if review.error}<p class="mt-3 rounded-md border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive" role="alert">Could not complete the check or confirm the action. {#if review.executionAttempted}Refresh this resource before retrying. {/if}{review.error}</p>{/if}
			{#if reviewBusy}<div class="flex items-center gap-2 py-8 text-sm text-muted-foreground"><Spinner />{review.status === "executing" ? "Applying the reviewed action" : "Checking with the API server"}</div>{:else if review.preview}
				<div class="mt-4 rounded-md border bg-muted/15">
					<div class="border-b px-3 py-2 text-xs font-medium">API server result</div>
					<pre class="max-h-[50vh] overflow-auto p-3 font-mono text-xs leading-5" aria-label="Server result diff">{#each diffLines as line}<span class={diffLineClassName(line.type)}>{line.text}{"\n"}</span>{/each}</pre>
				</div>
				{#if review.action === "delete"}<label class="mt-4 grid gap-1 text-xs">Type {review.name} to confirm deletion<Input aria-label="Type resource name to confirm" autocomplete="off" value={review.confirmation} oninput={(event: Event & { currentTarget: HTMLInputElement }) => session.confirm(event.currentTarget.value)} /></label>{/if}
			{/if}
			<div class="mt-4 flex flex-wrap items-center justify-end gap-2">
				{#if review.skipRequested}<span class="mr-auto text-xs text-muted-foreground">Successful review dialogs are skipped in Settings. Dry-run still runs.</span>{/if}
				<Button variant="outline" disabled={review.status === "executing"} onclick={() => session.closeReview()}>{reviewBusy ? "Cancel check" : "Cancel"}</Button>
				{#if review.preview}<Button variant={review.action === "delete" ? "destructive" : "default"} disabled={!review.canExecute} onclick={() => session.execute()}>{review.status === "executing" ? "Applying…" : review.action === "delete" ? "Delete" : "Apply"}</Button>{/if}
			</div>
		</DialogContent>
	</Dialog>
{/if}
