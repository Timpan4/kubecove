<script module lang="ts">
	const loadResourceDetail = () => import("@/features/resource-detail/ResourceDetailPanel.svelte");
</script>

<script lang="ts">
	import { Pin, X } from "lucide-svelte";
	import DeferredSurface from "@/components/DeferredSurface.svelte";
	import { Button } from "@/components/ui/svelte";
	import type { ArgoApplicationSummary } from "@/lib/gitops-types";
	import type { PathStateResourceDetailState } from "@/lib/path-state";
	import { resourceKey } from "@/lib/resource-identity";
	import type { TauriClient } from "@/lib/tauri";
	import type { ResourceSummary } from "@/lib/types";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";

	let {
		client,
		resource,
		pinned,
		argoApplicationSummary,
		workspaceReadContext,
		initialPathState,
		onPinToggle,
		onClose,
		onOpenHelmRelease,
		onPathStateChange,
	}: {
		client: TauriClient;
		resource: ResourceSummary;
		pinned: boolean;
		argoApplicationSummary: ArgoApplicationSummary | null;
		workspaceReadContext: WorkspaceReadContext;
		initialPathState: PathStateResourceDetailState | null;
		onPinToggle: () => void;
		onClose: () => void;
		onOpenHelmRelease: (releaseName: string, namespace?: string | null) => void;
		onPathStateChange: (state: PathStateResourceDetailState) => void;
	} = $props();
</script>

<section class="flex h-full min-h-0 flex-col bg-surface-1">
	<header class="flex shrink-0 items-start justify-between gap-3 border-b px-4 py-3">
		<div class="min-w-0">
			<h2 class="truncate text-sm font-semibold">{resource.name}</h2>
			<div class="truncate text-xs text-muted-foreground">
				{resource.kind} / {resource.namespace ?? "cluster"}
			</div>
		</div>
		<div class="flex shrink-0 items-center gap-1">
			<Button
				type="button"
				variant="ghost"
				size="icon"
				class="size-7 text-muted-foreground"
				aria-label={`${pinned ? "Unpin" : "Pin"} ${resource.kind} ${resource.name}`}
				onclick={onPinToggle}
			>
				<Pin class={pinned ? "fill-current" : ""} />
			</Button>
			<Button
				type="button"
				variant="ghost"
				size="icon"
				class="size-7 text-muted-foreground"
				aria-label="Close resource details"
				onclick={onClose}
			>
				<X />
			</Button>
		</div>
	</header>
	<div class="min-h-0 flex-1 overflow-y-auto overscroll-contain p-4">
		{#key resourceKey(resource)}
			<DeferredSurface load={loadResourceDetail} label="resource details">
				{#snippet children(ResourceDetailPanel)}
					<ResourceDetailPanel
						{client}
						{resource}
						{argoApplicationSummary}
						{workspaceReadContext}
						{onOpenHelmRelease}
						{initialPathState}
						{onPathStateChange}
					/>
				{/snippet}
			</DeferredSurface>
		{/key}
	</div>
</section>
