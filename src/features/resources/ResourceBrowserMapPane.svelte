<script module lang="ts">
	let ownershipMapImport: Promise<typeof import("./OwnershipMap.svelte")> | undefined;

	function loadOwnershipMap() {
		return (ownershipMapImport ??= import("./OwnershipMap.svelte"));
	}

	function retryOwnershipMapLoad() {
		ownershipMapImport = undefined;
		return loadOwnershipMap();
	}
</script>

<script lang="ts">
	import type { ComponentProps } from "svelte";
	import { GitBranch, PanelRightOpen } from "lucide-svelte";
	import { Button, Spinner } from "@/components/ui/svelte";
	import { loadingDelay } from "@/lib/loading-delay.svelte";
	import { shouldLoadOwnershipMap } from "./resourceBrowserModel";
	import type OwnershipMap from "./OwnershipMap.svelte";

	let {
		mapPanelOpen = $bindable(), mapPanelElement = $bindable(), onMapToggle,
		...options
	}: ComponentProps<typeof OwnershipMap> & {
		mapPanelOpen: boolean;
		mapPanelElement: HTMLDivElement | undefined;
	} = $props();

	let OwnershipMapComponent = $state<typeof import("./OwnershipMap.svelte").default | null>(null);
	let ownershipMapLoadError = $state<unknown>(null);
	const ownershipMapLoading = loadingDelay(() => !OwnershipMapComponent && !ownershipMapLoadError);
	$effect(() => {
		if (
			!shouldLoadOwnershipMap(
				mapPanelOpen,
				Boolean(OwnershipMapComponent),
				Boolean(ownershipMapLoadError),
			)
		) return;
		void loadOwnershipMap()
			.then((module) => {
				OwnershipMapComponent = module.default;
			})
			.catch((cause: unknown) => {
				ownershipMapLoadError = cause;
			});
	});

	function retryOwnershipMap() {
		ownershipMapLoadError = null;
		void retryOwnershipMapLoad()
			.then((module) => {
				OwnershipMapComponent = module.default;
			})
			.catch((cause: unknown) => {
				ownershipMapLoadError = cause;
			});
	}

</script>

{#if mapPanelOpen}
	<div class="h-full min-h-[400px] min-w-0" bind:this={mapPanelElement} tabindex="-1">
		{#if OwnershipMapComponent && !ownershipMapLoading.active}
			<OwnershipMapComponent {...options} {onMapToggle} />
		{:else if ownershipMapLoadError}
			<div class="flex h-full items-center justify-center gap-2 text-xs text-muted-foreground">
				<span>Failed to load ownership map.</span>
				<Button type="button" variant="outline" size="sm" onclick={retryOwnershipMap}>Retry</Button>
			</div>
		{:else}
			<div class="flex h-full items-center justify-center gap-2 text-xs text-muted-foreground">
				{#if ownershipMapLoading.shown}
					<Spinner />
					<span>Loading ownership map</span>
				{/if}
			</div>
		{/if}
	</div>
{:else}
	<aside
		class="flex h-full min-h-[400px] w-12 shrink-0 flex-col items-center overflow-hidden rounded-lg border bg-surface-1 shadow-sm"
	>
		<div class="flex w-full justify-center border-b p-2">
			<Button
				type="button"
				variant="ghost"
				size="icon"
				class="size-7"
				onclick={() => (mapPanelOpen = true)}
				aria-label="Show ownership map"
			>
				<PanelRightOpen />
			</Button>
		</div>
		<Button
			type="button"
			variant="ghost"
			class="h-auto min-h-0 w-full flex-1 flex-col gap-2 rounded-none border-0 px-2 py-3 text-muted-foreground hover:bg-transparent hover:text-foreground"
			onclick={() => (mapPanelOpen = true)}
			aria-label="Show ownership map"
		>
			<GitBranch class="size-4 shrink-0" />
			<span class="[writing-mode:vertical-rl] text-xs font-semibold">Map</span>
		</Button>
	</aside>
{/if}
