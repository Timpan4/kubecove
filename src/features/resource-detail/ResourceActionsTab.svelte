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
	import {
		executeResourceAction,
		getResourceActionState,
		previewResourceAction,
		type TauriClient,
	} from "@/lib/tauri";
	import type {
		ResourceActionEdits,
		ResourceActionPreview,
		ResourceActionRequest,
		ResourceActionState,
		ResourceSummary,
	} from "@/lib/types";
	import { settingsStore } from "@/lib/settings-store";
	import { queryKeys } from "@/lib/queryKeys";
	import { buildCompactUnifiedDiff, diffLineClassName } from "./yamlTabDiff";
	import {
		actionTarget,
		fallbackPermission,
		hasPendingResourceActionEdits,
		permissionForAction,
		resourceActionEditCount,
		resourceActionRows,
		type ResourceActionId,
		type ResourceActionRow,
		type ResourceActionRowId,
	} from "./resource-actions-model";
	import { getErrorMessage } from "./helpers";

	type MetadataName = "labels" | "annotations";
	type ContainerDraft = NonNullable<ResourceActionEdits["containers"]>[number];
	interface ReviewState {
		action: ResourceActionId;
		resource: ResourceSummary;
		request: ResourceActionRequest;
		pendingEdits: ResourceActionEdits;
		preview: ResourceActionPreview | null;
		error: string | null;
	executionAttempted?: boolean;
		busy: boolean;
		phase: "preview" | "execute";
		attempt: number;
		confirmedName: string;
		revision?: number;
	}

	let {
		client,
		resource,
		kubeconfigSourceKey,
	}: {
		client: TauriClient;
		resource: ResourceSummary;
		kubeconfigSourceKey?: string;
	} = $props();

	const queryClient = useQueryClient();
	let actionState = $state<ResourceActionState | null>(null);
	let loadError = $state<string | null>(null);
	let loading = $state(true);
	let filter = $state("");
	let expanded = $state<ResourceActionRowId | null>(null);
	let selectedRevision = $state<number | null>(null);
	let edits = $state<ResourceActionEdits>({});
	let newEnvironment = $state<Record<string, { name: string; value: string }>>({});
	let newQuantities = $state<Record<string, Partial<Record<"requests" | "limits", { name: string; value: string }>>>>({});
	let newMetadata = $state<Record<MetadataName, { name: string; value: string }>>({
		labels: { name: "", value: "" },
		annotations: { name: "", value: "" },
	});
	let review = $state<ReviewState | null>(null);
	let reviewOpen = $state(false);
	let actionResult = $state<string | null>(null);
	let inlineStatus = $state<string | null>(null);
	let generation = 0;
	let reviewAttempt = 0;

	const resourceIdentity = $derived(JSON.stringify({ target: actionTarget(resource), kubeconfigSourceKey }));
	const rows = $derived(resourceActionRows(resource));
	const visibleRows = $derived.by(() => {
		const query = filter.trim().toLocaleLowerCase();
		return rows.filter((row) => !query || `${row.label} ${row.group} ${row.search}`.toLocaleLowerCase().includes(query));
	});
	const groups = $derived([...new Set(visibleRows.map((row) => row.group))]);
	const pending = $derived(hasPendingResourceActionEdits(edits));
	const skipReview = $derived($settingsStore.skipChangeReview);
	const diffLines = $derived(review?.preview ? buildCompactUnifiedDiff(review.preview.beforeYaml, review.preview.afterYaml) : []);

	function captureTarget() {
		return { ...actionTarget(resource), kubeconfigEnvVar: kubeconfigSourceKey };
	}

	async function loadState(expectedGeneration: number): Promise<ResourceActionState | null> {
		try {
			const next = await getResourceActionState(client, captureTarget());
			if (generation !== expectedGeneration) return null;
			actionState = next;
			loadError = null;
			return next;
		} catch (cause) {
			if (generation === expectedGeneration) {
				actionState = null;
				loadError = getErrorMessage(cause);
			}
			return null;
		} finally {
			if (generation === expectedGeneration) loading = false;
		}
	}

	$effect(() => {
		const identity = resourceIdentity;
		const currentGeneration = ++generation;
		actionState = null;
		loadError = null;
		loading = true;
		filter = "";
		expanded = null;
		selectedRevision = null;
		edits = {};
		newEnvironment = {};
		newQuantities = {};
		newMetadata = { labels: { name: "", value: "" }, annotations: { name: "", value: "" } };
		review = null;
		reviewOpen = false;
		actionResult = null;
		inlineStatus = null;
		void identity;
		untrack(() => {
			if (rows.length > 0) void loadState(currentGeneration);
			else loading = false;
		});
		return () => {
			generation += 1;
		};
	});
	function retryState() {
		loading = true;
		loadError = null;
		void loadState(generation);
	}

	function handleReviewOpenChange(open: boolean) {
		if (open) {
			reviewOpen = true;
			return;
		}
		closeReview();
	}

	function allowed(action: ResourceActionId): boolean {
		return permissionForAction(actionState, action)?.allowed === true;
	}

	function setContainerEdit(name: string, updater: (current: ContainerDraft) => ContainerDraft) {
		const current = edits.containers ?? [];
		const previous = current.find((container) => container.name === name) ?? emptyContainerEdit(name);
		const next = updater(previous);
		edits = {
			...edits,
			containers: [...current.filter((container) => container.name !== name), next],
		};
	}

	function containerEdit(name: string): ContainerDraft {
		return edits.containers?.find((container) => container.name === name) ?? emptyContainerEdit(name);
	}

	function emptyContainerEdit(name: string): ContainerDraft {
		return { name, env: [], requests: {}, limits: {} };
	}

	function setResourceMap(name: string, field: "requests" | "limits", key: string, value: string | null) {
		setContainerEdit(name, (current) => ({ ...current, [field]: { ...current[field], [key]: value } }));
	}

	function displayedQuantities(name: string, field: "requests" | "limits") {
		const container = actionState?.containers.find((item) => item.name === name);
		const base = container?.[field] ?? {};
		const changes = containerEdit(name)[field] ?? {};
		return [...new Set([...Object.keys(base), ...Object.keys(changes)])].map((key) => ({
			key,
			value: Object.hasOwn(changes, key) && changes[key] !== null ? changes[key] ?? "" : base[key] ?? "",
			removed: changes[key] === null,
		}));
	}

	function removeQuantity(name: string, field: "requests" | "limits", key: string) {
		const container = actionState?.containers.find((item) => item.name === name);
		if (container && Object.hasOwn(container[field], key)) {
			setResourceMap(name, field, key, null);
			return;
		}
		setContainerEdit(name, (current) => {
			const map = { ...current[field] };
			delete map[key];
			return { ...current, [field]: map };
		});
	}

	function addQuantity(name: string, field: "requests" | "limits") {
		const draft = newQuantities[name]?.[field] ?? { name: "", value: "" };
		if (!draft.name.trim()) return;
		setResourceMap(name, field, draft.name.trim(), draft.value);
		newQuantities = { ...newQuantities, [name]: { ...newQuantities[name], [field]: { name: "", value: "" } } };
	}

	function setEnvironment(name: string, variable: string, value: string) {
		setContainerEdit(name, (current) => {
			const env = current.env ?? [];
			return {
				...current,
				env: [...env.filter((item) => item.name !== variable), { name: variable, value }],
			};
		});
	}

	function removeEnvironment(name: string, variable: string) {
		const exists = actionState?.containers.find((container) => container.name === name)?.env.some((item) => item.name === variable);
		setContainerEdit(name, (current) => {
			const env = current.env ?? [];
			return {
				...current,
				env: exists
					? [...env.filter((item) => item.name !== variable), { name: variable, remove: true }]
					: env.filter((item) => item.name !== variable),
			};
		});
	}

	function renameEnvironment(name: string, previousName: string, nextName: string) {
		setContainerEdit(name, (current) => ({
			...current,
			env: (current.env ?? []).map((item) => item.name === previousName ? { ...item, name: nextName } : item),
		}));
	}

	function restoreEnvironment(name: string, variable: string) {
		setContainerEdit(name, (current) => ({
			...current,
			env: (current.env ?? []).filter((item) => item.name !== variable),
		}));
	}

	function displayedEnvironment(containerName: string) {
		const container = actionState?.containers.find((item) => item.name === containerName);
		if (!container) return [];
		const changes = containerEdit(containerName).env ?? [];
		const rows = container.env.map((variable) => {
			const change = changes.find((item) => item.name === variable.name);
			return {
				name: variable.name,
				value: change?.remove ? variable.value ?? "" : change?.value ?? variable.value ?? "",
				referenced: variable.referenced,
				removed: Boolean(change?.remove),
				isNew: false,
			};
		});
		for (const change of changes) {
			if (change.remove || container.env.some((variable) => variable.name === change.name)) continue;
			rows.push({ name: change.name, value: change.value ?? "", referenced: false, removed: false, isNew: true });
		}
		return rows;
	}

	function addEnvironment(name: string) {
		const draft = newEnvironment[name] ?? { name: "", value: "" };
		if (!draft.name.trim() || actionState?.containers.find((container) => container.name === name)?.env.some((item) => item.name === draft.name.trim())) return;
		setEnvironment(name, draft.name.trim(), draft.value);
		newEnvironment = { ...newEnvironment, [name]: { name: "", value: "" } };
	}

	function setMetadata(name: MetadataName, key: string, value: string | null) {
		edits = { ...edits, [name]: { ...edits[name], [key]: value } };
	}

	function addMetadata(name: MetadataName) {
		const draft = newMetadata[name];
		if (!draft.name.trim()) return;
		setMetadata(name, draft.name.trim(), draft.value);
		newMetadata = { ...newMetadata, [name]: { name: "", value: "" } };
	}

	function displayedMetadata(name: MetadataName): Array<[string, string, boolean]> {
		const base = actionState?.[name] ?? {};
		const changes = edits[name] ?? {};
		const keys = new Set([...Object.keys(base), ...Object.keys(changes)]);
		return [...keys]
			.filter((key) => changes[key] !== null)
			.map((key) => [key, changes[key] ?? base[key] ?? "", Object.hasOwn(changes, key)]);
	}

	function actionForRow(row: ResourceActionRow): ResourceActionId {
		if (row.id === "rollout" && actionState?.paused) return "resume";
		return row.action;
	}

	function requestedEdits(): ResourceActionEdits {
		return structuredClone($state.snapshot(edits));
	}

	function requestFor(action: ResourceActionId, revision?: number): ResourceActionRequest {
		const request: ResourceActionRequest = {
			...actionTarget(resource),
			action,
			confirmed: false,
			kubeconfigEnvVar: kubeconfigSourceKey,
		};
		if (action === "edit" && pending) request.edits = requestedEdits();
		if (revision !== undefined) request.revision = revision;
		return request;
	}

	function openReview(action: ResourceActionId, revision?: number) {
		if (!allowed(action)) return;
		actionResult = null;
		const skipping = skipReview && action !== "delete";
		reviewOpen = !skipping;
		inlineStatus = skipping ? "Checking with the API server…" : null;
		const attempt = ++reviewAttempt;
		const request = requestFor(action, revision);
		review = {
			action,
			resource: $state.snapshot(resource),
			request,
			pendingEdits: requestedEdits(),
			preview: null,
			error: null,
			busy: true,
			phase: "preview",
			attempt,
			confirmedName: "",
			revision,
		};
		const currentGeneration = generation;
		void previewResourceAction(client, request)
			.then(async (preview) => {
				if (currentGeneration !== generation || review?.attempt !== attempt) return;
				if (skipping && preview.beforeYaml !== preview.afterYaml) {
					inlineStatus = "Applying the reviewed action…";
					await executePreview(preview, currentGeneration, attempt);
					return;
				}
				inlineStatus = null;
				reviewOpen = true;
				if (review) review = { ...review, preview, busy: false };
			})
			.catch((cause: unknown) => {
				if (currentGeneration !== generation || review?.attempt !== attempt) return;
				inlineStatus = null;
				reviewOpen = true;
				review = {
					...review,
					error: getErrorMessage(cause),
					busy: false,
				};
			});
	}

	async function invalidateSuccessfulAction(request: ResourceActionRequest, targetResource: ResourceSummary) {
		const target = request;
		const source = queryKeys.resources(target.clusterContext, [], target.kubeconfigEnvVar)[1];
		const detailPrefix = queryKeys.resourceDetails(targetResource, null, target.kubeconfigEnvVar).slice(0, 8);
		const yamlPrefix = queryKeys.resourceYaml(targetResource, null, target.kubeconfigEnvVar).slice(0, 8);
		await Promise.all([
			queryClient.invalidateQueries({ predicate: (query) => detailPrefix.every((part, index) => query.queryKey[index] === part) }),
			queryClient.invalidateQueries({ predicate: (query) => yamlPrefix.every((part, index) => query.queryKey[index] === part) }),
			queryClient.invalidateQueries({ queryKey: queryKeys.deploymentRevisions(targetResource, target.kubeconfigEnvVar) }),
			queryClient.invalidateQueries({
				predicate: (query) =>
					(query.queryKey[0] === "resources" || query.queryKey[0] === "resource-topology") &&
					query.queryKey[1] === source && query.queryKey[2] === target.clusterContext,
			}),
		]);
	}

	async function executePreview(
		preview: ResourceActionPreview,
		currentGeneration: number,
		attempt: number,
	) {
		const currentReview = review;
		if (!currentReview || currentReview.attempt !== attempt || currentReview.phase !== "preview") return;
		const { action, request, pendingEdits } = currentReview;
		review = { ...currentReview, preview, busy: true, phase: "execute", error: null };
		try {
			const executionRequest: ResourceActionRequest = {
				...request,
				expectedUid: preview.uid,
				expectedResourceVersion: preview.resourceVersion,
				confirmed: true,
			};
			if (action === "rollback") {
				executionRequest.expectedRevisionUid = preview.revisionUid;
				executionRequest.expectedRevisionResourceVersion = preview.revisionResourceVersion;
			}
			const result = await executeResourceAction(client, executionRequest);
			await invalidateSuccessfulAction(request, currentReview.resource);
			if (generation !== currentGeneration || review?.attempt !== attempt) return;
			inlineStatus = null;
			actionResult = result.effect;
			if (action === "edit" && JSON.stringify(edits) === JSON.stringify(request.edits ?? {})) {
				edits = {};
				newEnvironment = {};
				newQuantities = {};
			}
			review = null;
			reviewOpen = false;
			const refreshed = await loadState(currentGeneration);
			if (generation !== currentGeneration || reviewAttempt !== attempt || !refreshed) return;
			if (action !== "edit" && hasPendingResourceActionEdits(pendingEdits)) edits = pendingEdits;
		} catch (cause) {
			if (generation !== currentGeneration || review?.attempt !== attempt) return;
			inlineStatus = null;
			reviewOpen = true;
			review = {
				...review,
				error: getErrorMessage(cause),
				executionAttempted: true,
				busy: false,
				phase: "preview",
			};
		}
	}

	function closeReview() {
		if (review?.phase === "execute") return;
		reviewAttempt += 1;
		review = null;
		reviewOpen = false;
	}

	function revertRow(row: ResourceActionRow) {
		if (row.id === "replicas") {
			const { replicas: _replicas, ...rest } = edits;
			edits = rest;
		} else if (row.id === "image" || row.id === "environment" || row.id === "resources") {
			const field = row.id === "image" ? "image" : row.id === "environment" ? "env" : "resources";
			if (row.id === "environment") newEnvironment = {};
			if (row.id === "resources") newQuantities = {};
			edits = {
				...edits,
				containers: (edits.containers ?? [])
					.map((container) => {
						if (field === "image") return { ...container, image: undefined };
						if (field === "env") return { ...container, env: [] };
						return { ...container, requests: {}, limits: {} };
					})
					.filter((container) => container.image !== undefined || container.env?.length || Object.keys(container.requests ?? {}).length || Object.keys(container.limits ?? {}).length),
			};
		} else if (row.id === "labels" || row.id === "annotations") {
			const { [row.id]: _metadata, ...rest } = edits;
			edits = rest;
		}
	}

	function pendingLabel(row: ResourceActionRow): string {
		if (row.id === "replicas" && edits.replicas !== undefined) return `Pending: ${edits.replicas}`;
		if (row.id === "image") {
			const changed = edits.containers?.filter((container) => container.image !== undefined) ?? [];
			return changed.length ? `Pending: ${changed.map((item) => `${item.name} ${item.image}`).join(", ")}` : "";
		}
		if (row.id === "environment") {
			const count = edits.containers?.reduce((total, container) => total + (container.env?.length ?? 0), 0) ?? 0;
			return count ? `Pending: ${count} environment change${count === 1 ? "" : "s"}` : "";
		}
		if (row.id === "resources") {
			const count = edits.containers?.reduce((total, container) => total + Object.keys(container.requests ?? {}).length + Object.keys(container.limits ?? {}).length, 0) ?? 0;
			return count ? `Pending: ${count} resource change${count === 1 ? "" : "s"}` : "";
		}
		if (row.id === "labels" && Object.keys(edits.labels ?? {}).length) {
			const count = Object.keys(edits.labels ?? {}).length;
			return `Pending: ${count} label change${count === 1 ? "" : "s"}`;
		}
		if (row.id === "annotations" && Object.keys(edits.annotations ?? {}).length) {
			const count = Object.keys(edits.annotations ?? {}).length;
			return `Pending: ${count} annotation change${count === 1 ? "" : "s"}`;
		}
		return "";
	}

	function renderCurrent(row: ResourceActionRow): string {
		if (!actionState) return "";
		if (row.id === "replicas") return `${actionState.replicas ?? 0} desired`;
		if (row.id === "rollout") return actionState.paused ? "Paused" : "Running";
		if (row.id === "restart") return "Replace Pods gradually";
		if (row.id === "rollback") return actionState.revisions.length ? `${actionState.revisions.length} available revisions` : "No owned revisions found";
		if (row.id === "image") return actionState.containers.map((container) => `${container.name}: ${container.image}`).join(", ");
		if (row.id === "environment") return actionState.containers.map((container) => `${container.name}: ${container.env.map((variable) => variable.name).join(", ") || "no variables"}`).join("; ");
		if (row.id === "resources") return actionState.containers.map((container) => `${container.name}: requests ${Object.entries(container.requests).map(([key, value]) => `${key} ${value}`).join(", ") || "none"}; limits ${Object.entries(container.limits).map(([key, value]) => `${key} ${value}`).join(", ") || "none"}`).join("; ");
		if (row.id === "labels") return Object.entries(actionState.labels).map(([key, value]) => `${key}=${value}`).join(", ") || "No labels";
		if (row.id === "annotations") return Object.entries(actionState.annotations).map(([key, value]) => `${key}=${value}`).join(", ") || "No annotations";
		return `Deletes this ${resource.kind} and its dependents where Kubernetes applies them.`;
	}
</script>

<div class="flex h-full min-h-0 flex-col overflow-auto text-sm">
	<div class="flex flex-wrap items-center justify-between gap-3 border-b pb-3">
		<div class="flex flex-wrap items-center gap-2 text-xs">
			{#if actionState?.replicas !== undefined}<span class="rounded-full border bg-muted/30 px-2 py-1 tabular-nums">Desired {actionState.replicas}</span>{/if}
			{#if actionState?.paused !== undefined}<span class="rounded-full border bg-muted/30 px-2 py-1">{actionState.paused ? "Paused" : "Running"}</span>{/if}
			{#if actionState?.revisions.length}<span class="rounded-full border bg-muted/30 px-2 py-1">{actionState.revisions.length} revisions</span>{/if}
			{#if loading}<span class="inline-flex items-center gap-2 text-muted-foreground"><Spinner />Loading action permissions</span>{/if}
		</div>
		<label class="flex h-8 w-56 items-center gap-2 rounded-md border bg-muted/20 px-2 text-muted-foreground focus-within:ring-2 focus-within:ring-ring">
			<Search class="size-3.5 shrink-0" aria-hidden="true" />
			<Input class="h-full border-0 bg-transparent p-0 shadow-none focus-visible:ring-0" aria-label="Resource action filter" placeholder="Filter actions" bind:value={filter} />
			<kbd class="text-[0.65rem]">/</kbd>
		</label>
	</div>

	{#if loadError}<div class="flex flex-wrap items-center justify-between gap-2 border-b border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive" role="status"><span>Unable to load action state and permissions. Actions remain locked. {loadError}</span><Button variant="ghost" size="sm" disabled={loading} onclick={retryState}>Retry permission check</Button></div>{/if}
	{#if actionResult}<p class="border-b border-emerald-500/30 bg-emerald-500/5 px-3 py-2 text-xs text-emerald-300" role="status">{actionResult}</p>{/if}
	{#if inlineStatus}<p class="flex items-center gap-2 border-b px-3 py-2 text-xs text-muted-foreground" role="status"><Spinner />{inlineStatus}</p>{/if}
	{#if rows.length === 0}<p class="py-8 text-muted-foreground">Actions are not available for this resource.</p>{/if}
	{#if rows.length > 0 && visibleRows.length === 0}<p class="py-8 text-muted-foreground">No action matches “{filter}”.</p>{/if}

	{#each groups as group (group)}
		{#if group}<h3 class="mt-4 border-b px-1 pb-2 text-xs font-semibold text-muted-foreground">{group}</h3>{/if}
		<div class="divide-y">
			{#each visibleRows.filter((row) => row.group === group) as row (row.id)}
				{@const rowAction = actionForRow(row)}
			{@const permission = permissionForAction(actionState, rowAction)}
				{@const canAct = permission?.allowed === true}
				{@const isExpanded = expanded === row.id}
				{@const pendingText = pendingLabel(row)}
				<article class="py-3 pl-1" data-resource-action={row.id}>
					<div class="flex flex-wrap items-start justify-between gap-3">
						<div class="min-w-0 flex-1">
							<div class="flex items-center gap-2 font-medium">{row.label}{#if pendingText}<span class="rounded-full bg-sky-500/10 px-2 py-0.5 text-[0.7rem] font-normal text-sky-300">Pending</span>{/if}</div>
							<p class="mt-1 break-words text-xs text-muted-foreground">{renderCurrent(row)}</p>
							{#if pendingText}<p class="mt-1 text-xs text-sky-300">{pendingText}</p>{/if}
							{#if !canAct}<p class="mt-1 flex items-center gap-1 text-xs text-muted-foreground"><LockKeyhole class="size-3.5" aria-hidden="true" />Needs {permission?.requiredPermission || fallbackPermission(rowAction, resource.kind)}{#if permission?.reason}<span>· {permission.reason}</span>{:else if !permission}<span>· Permission has not been confirmed</span>{/if}</p>{/if}
						</div>
						<div class="flex shrink-0 items-center gap-2">
							{#if pendingText && row.action === "edit"}<Button variant="ghost" size="sm" onclick={() => revertRow(row)}>Revert</Button>{/if}
							{#if row.id === "delete"}
								<Button variant="destructive" size="sm" disabled={!canAct || loading} onclick={() => openReview("delete")}>Delete</Button>
							{:else if row.id === "restart"}
								<Button variant="outline" size="sm" disabled={!canAct || loading} onclick={() => openReview("restart")}>Restart</Button>
							{:else if row.id === "rollout"}
								<Button variant="outline" size="sm" disabled={!canAct || loading} onclick={() => openReview(rowAction)}>{actionState?.paused ? "Resume" : "Pause"}</Button>
							{:else if row.id === "rollback"}
								<Button variant="outline" size="sm" disabled={!canAct || loading || actionState?.revisions.length === 0} onclick={() => (expanded = isExpanded ? null : row.id)}>{isExpanded ? "Close" : "Roll back"}</Button>
							{:else}
								<Button variant="outline" size="sm" disabled={!canAct || loading} aria-expanded={isExpanded} onclick={() => (expanded = isExpanded ? null : row.id)}>{isExpanded ? "Done" : "Edit"}</Button>
							{/if}
						</div>
					</div>

					{#if isExpanded && canAct && row.id === "replicas"}
						<div class="mt-3 max-w-56"><label class="grid gap-1 text-xs text-muted-foreground">Desired replicas<Input type="number" min="0" step="1" aria-label="Desired replicas" value={String(edits.replicas ?? actionState?.replicas ?? 0)} oninput={(event: Event & { currentTarget: HTMLInputElement }) => { const value = Number(event.currentTarget.value); if (Number.isSafeInteger(value) && value >= 0) edits = { ...edits, replicas: value }; }} /></label></div>
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
							<Button size="sm" disabled={selectedRevision === null} onclick={() => selectedRevision !== null && openReview("rollback", selectedRevision)}>Review rollback</Button>
						</div>
					{/if}
					{#if isExpanded && canAct && ["image", "environment", "resources"].includes(row.id) && actionState}
						<div class="mt-3 space-y-4 border-l pl-3">
							{#each actionState.containers as container (container.name)}
								<section class="space-y-2">
									<h4 class="text-xs font-semibold">{container.name}</h4>
									{#if row.id === "image"}
										<label class="grid max-w-xl gap-1 text-xs text-muted-foreground">Container image<Input aria-label="Container image" value={containerEdit(container.name).image ?? container.image} oninput={(event: Event & { currentTarget: HTMLInputElement }) => setContainerEdit(container.name, (current) => ({ ...current, image: event.currentTarget.value }))} /></label>
									{:else if row.id === "environment"}
										<div class="space-y-2">
											{#each displayedEnvironment(container.name) as variable, index (variable.isNew ? `new:${index}` : variable.name)}
												<div class="grid gap-2 sm:grid-cols-[minmax(8rem,0.7fr)_minmax(10rem,1.3fr)_auto]">
													<Input aria-label={`Environment name ${container.name} ${variable.name}`} value={variable.name} disabled={!variable.isNew} oninput={(event: Event & { currentTarget: HTMLInputElement }) => renameEnvironment(container.name, variable.name, event.currentTarget.value)} />
													<Input aria-label={`Environment value ${container.name} ${variable.name}`} type="text" placeholder={variable.referenced ? "Value from Secret or ConfigMap" : "Value"} value={variable.value} disabled={variable.removed} oninput={(event: Event & { currentTarget: HTMLInputElement }) => setEnvironment(container.name, variable.name, event.currentTarget.value)} />
													<Button variant="ghost" size="sm" onclick={() => variable.removed ? restoreEnvironment(container.name, variable.name) : removeEnvironment(container.name, variable.name)}>{variable.removed ? "Restore" : "Remove"}</Button>
												</div>
											{/each}
											<div class="grid gap-2 sm:grid-cols-[minmax(8rem,0.7fr)_minmax(10rem,1.3fr)_auto]">
											<Input aria-label={`New environment name ${container.name}`} placeholder="Variable name" value={newEnvironment[container.name]?.name ?? ""} oninput={(event: Event & { currentTarget: HTMLInputElement }) => (newEnvironment = { ...newEnvironment, [container.name]: { name: event.currentTarget.value, value: newEnvironment[container.name]?.value ?? "" } })} />
												<Input aria-label={`New environment value ${container.name}`} placeholder="Value" value={newEnvironment[container.name]?.value ?? ""} oninput={(event: Event & { currentTarget: HTMLInputElement }) => (newEnvironment = { ...newEnvironment, [container.name]: { name: newEnvironment[container.name]?.name ?? "", value: event.currentTarget.value } })} />
												<Button variant="outline" size="sm" disabled={!newEnvironment[container.name]?.name.trim()} onclick={() => addEnvironment(container.name)}>Add</Button>
											</div>
										</div>
										{:else}
										<div class="space-y-3">
											{#each ["requests", "limits"] as field (field)}
												{@const quantityField = field as "requests" | "limits"}
												<div class="space-y-2">
													<h5 class="text-xs font-medium">{quantityField === "requests" ? "Requests" : "Limits"}</h5>
													{#each displayedQuantities(container.name, quantityField) as quantity (quantity.key)}
														<div class="grid gap-2 sm:grid-cols-[minmax(8rem,0.7fr)_minmax(10rem,1.3fr)_auto]">
															<Input aria-label={`${quantityField === "requests" ? "Request" : "Limit"} ${container.name} ${quantity.key}`} value={quantity.key} disabled />
													<Input aria-label={`${quantityField === "requests" ? "Request" : "Limit"} value ${container.name} ${quantity.key}`} value={quantity.value} oninput={(event: Event & { currentTarget: HTMLInputElement }) => setResourceMap(container.name, quantityField, quantity.key, event.currentTarget.value)} />
														<Button variant="ghost" size="sm" onclick={() => quantity.removed ? setResourceMap(container.name, quantityField, quantity.key, actionState?.containers.find((item) => item.name === container.name)?.[quantityField][quantity.key] ?? "") : removeQuantity(container.name, quantityField, quantity.key)}>{quantity.removed ? "Restore" : "Remove"}</Button>
														</div>
													{/each}
													<div class="grid gap-2 sm:grid-cols-[minmax(8rem,0.7fr)_minmax(10rem,1.3fr)_auto]">
									<Input aria-label={`New ${quantityField === "requests" ? "request" : "limit"} name ${container.name}`} placeholder="Resource name" value={newQuantities[container.name]?.[quantityField]?.name ?? ""} oninput={(event: Event & { currentTarget: HTMLInputElement }) => (newQuantities = { ...newQuantities, [container.name]: { ...newQuantities[container.name], [quantityField]: { name: event.currentTarget.value, value: newQuantities[container.name]?.[quantityField]?.value ?? "" } } })} />
									<Input aria-label={`New ${quantityField === "requests" ? "request" : "limit"} value ${container.name}`} placeholder="Quantity" value={newQuantities[container.name]?.[quantityField]?.value ?? ""} oninput={(event: Event & { currentTarget: HTMLInputElement }) => (newQuantities = { ...newQuantities, [container.name]: { ...newQuantities[container.name], [quantityField]: { name: newQuantities[container.name]?.[quantityField]?.name ?? "", value: event.currentTarget.value } } })} />
														<Button variant="outline" size="sm" disabled={!newQuantities[container.name]?.[quantityField]?.name.trim()} onclick={() => addQuantity(container.name, quantityField)}>Add</Button>
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
						{@const metadataName = row.id as MetadataName}
						<div class="mt-3 space-y-2 border-l pl-3">
							{#each displayedMetadata(metadataName) as [key, value, changed] (key)}
								<div class="grid gap-2 sm:grid-cols-[minmax(8rem,0.7fr)_minmax(10rem,1.3fr)_auto]">
									<Input aria-label={`${row.label} key ${key}`} value={key} disabled />
									<Input aria-label={`${row.label} value ${key}`} value={value} oninput={(event: Event & { currentTarget: HTMLInputElement }) => setMetadata(metadataName, key, event.currentTarget.value)} />
									<Button variant="ghost" size="sm" onclick={() => setMetadata(metadataName, key, null)}>{changed && edits[metadataName]?.[key] === null ? "Removed" : "Remove"}</Button>
								</div>
							{/each}
							<div class="grid gap-2 sm:grid-cols-[minmax(8rem,0.7fr)_minmax(10rem,1.3fr)_auto]">
								<Input aria-label={`New ${row.label.toLocaleLowerCase()} key`} placeholder="Key" value={newMetadata[metadataName].name} oninput={(event: Event & { currentTarget: HTMLInputElement }) => (newMetadata = { ...newMetadata, [metadataName]: { ...newMetadata[metadataName], name: event.currentTarget.value } })} />
								<Input aria-label={`New ${row.label.toLocaleLowerCase()} value`} placeholder="Value" value={newMetadata[metadataName].value} oninput={(event: Event & { currentTarget: HTMLInputElement }) => (newMetadata = { ...newMetadata, [metadataName]: { ...newMetadata[metadataName], value: event.currentTarget.value } })} />
								<Button variant="outline" size="sm" disabled={!newMetadata[metadataName].name.trim()} onclick={() => addMetadata(metadataName)}>Add</Button>
							</div>
						</div>
					{/if}
				</article>
			{/each}
		</div>
	{/each}

	{#if loadError && rows.length === 0}<p class="py-6 text-xs text-muted-foreground">The resource kind is outside the selected-resource action allowlist.</p>{/if}

	{#if pending}
		<div class="sticky bottom-0 mt-auto flex flex-wrap items-center justify-between gap-3 border-t bg-background/90 px-3 py-2 shadow-sm backdrop-blur">
			<div class="text-xs text-muted-foreground">{resourceActionEditCount(edits)} pending changes</div>
			<div class="flex items-center gap-2"><Button variant="ghost" size="sm" onclick={() => (edits = {})}>Discard</Button><Button size="sm" disabled={!allowed("edit") || loading} onclick={() => openReview("edit")}>Check changes</Button></div>
		</div>
	{/if}
</div>

{#if review}
	<Dialog open={reviewOpen} onOpenChange={handleReviewOpenChange}>
		<DialogContent class="max-h-[min(85vh,52rem)] max-w-4xl overflow-auto">
			<DialogHeader>
				<DialogTitle>Review {review.action === "delete" ? `deletion of ${review.request.name}` : `${review.action} for ${review.request.name}`}</DialogTitle>
				<DialogDescription>{review.preview?.effect ?? (review.phase === "execute" ? "Applying the reviewed action…" : review.busy ? "Checking this exact resource with the API server…" : "The API server could not validate this action.")}</DialogDescription>
			</DialogHeader>
			<div class="flex justify-end"><Button variant="ghost" size="sm" disabled={review.phase === "execute"} aria-label="Close review" onclick={closeReview}>{review.busy ? "Cancel check" : "Close review"}</Button></div>
			{#if review.error}<p class="mt-3 rounded-md border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive" role="alert">Could not complete the check or confirm the action. {#if review.executionAttempted}Refresh this resource before retrying. {/if}{review.error}</p>{/if}
			{#if review.busy}<div class="flex items-center gap-2 py-8 text-sm text-muted-foreground"><Spinner />{review.phase === "execute" ? "Applying the reviewed action" : "Checking with the API server"}</div>{:else if review.preview}
				<div class="mt-4 rounded-md border bg-muted/15">
					<div class="border-b px-3 py-2 text-xs font-medium">API server result</div>
					<pre class="max-h-[50vh] overflow-auto p-3 font-mono text-xs leading-5" aria-label="Server result diff">{#each diffLines as line}<span class={diffLineClassName(line.type)}>{line.text}{"\n"}</span>{/each}</pre>
				</div>
			{#if review.action === "delete"}<label class="mt-4 grid gap-1 text-xs">Type {review.request.name} to confirm deletion<Input aria-label="Type resource name to confirm" autocomplete="off" bind:value={review.confirmedName} /></label>{/if}
			{/if}
			<div class="mt-4 flex flex-wrap items-center justify-end gap-2">
			{#if skipReview && review.action !== "delete"}<span class="mr-auto text-xs text-muted-foreground">Successful review dialogs are skipped in Settings. Dry-run still runs.</span>{/if}
				<Button variant="outline" disabled={review.phase === "execute"} onclick={closeReview}>{review.busy ? "Cancel check" : "Cancel"}</Button>
				{#if review.preview}<Button variant={review.action === "delete" ? "destructive" : "default"} disabled={review.phase === "execute" || Boolean(review.error) || (review.action === "delete" && review.confirmedName !== review.request.name)} onclick={() => executePreview(review!.preview!, generation, review!.attempt)}>{review.phase === "execute" ? "Applying…" : review.action === "delete" ? "Delete" : "Apply"}</Button>{/if}
			</div>
		</DialogContent>
	</Dialog>
{/if}
