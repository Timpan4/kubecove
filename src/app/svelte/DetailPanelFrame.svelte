<script lang="ts">
	import { onMount, type Snippet } from "svelte";
	import { cnfast } from "@/lib/utils";
	import {
		detailPanelSizeFromKey,
		detailPanelSizeFromPointer,
		DETAIL_PANEL_DEFAULT_SIZE,
		DETAIL_PANEL_MIN_SIZE,
		MAIN_PANEL_MIN_SIZE,
	} from "./detailPanelFrameModel";

	let {
		detailOpen = false,
		detailDefaultSize = DETAIL_PANEL_DEFAULT_SIZE,
		detailMinSize = DETAIL_PANEL_MIN_SIZE,
		mainMinSize = MAIN_PANEL_MIN_SIZE,
		sizeKey = "default",
		class: className = "",
		children,
		detailPanel,
	}: {
		detailOpen?: boolean;
		detailDefaultSize?: number;
		detailMinSize?: number;
		mainMinSize?: number;
		sizeKey?: string;
		class?: string;
		children?: Snippet;
		detailPanel?: Snippet;
	} = $props();

	let frameElement = $state<HTMLDivElement | null>(null);
	let detailSize = $state(DETAIL_PANEL_DEFAULT_SIZE);
	let activeSizeKey = $state("");
	let stopResize: (() => void) | null = null;
	const mainSize = $derived(100 - detailSize);

	onMount(() => () => stopCurrentResize());

	$effect(() => {
		if (activeSizeKey === sizeKey) return;
		activeSizeKey = sizeKey;
		detailSize = detailDefaultSize;
	});

	function stopCurrentResize() {
		if (!stopResize) return;
		stopResize();
		stopResize = null;
	}

	function startResize(event: PointerEvent) {
		if (!frameElement) return;
		event.preventDefault();
		stopCurrentResize();
		const handleMove = (moveEvent: PointerEvent) => {
			if (!frameElement) return;
			detailSize = detailPanelSizeFromPointer(
				frameElement.getBoundingClientRect(),
				moveEvent.clientX,
				detailDefaultSize,
				detailMinSize,
				mainMinSize,
			);
		};
		const handleEnd = () => stopCurrentResize();
		window.addEventListener("pointermove", handleMove);
		window.addEventListener("pointerup", handleEnd);
		window.addEventListener("pointercancel", handleEnd);
		stopResize = () => {
			window.removeEventListener("pointermove", handleMove);
			window.removeEventListener("pointerup", handleEnd);
			window.removeEventListener("pointercancel", handleEnd);
		};
	}

	function handleResizeKeydown(event: KeyboardEvent) {
		const nextSize = detailPanelSizeFromKey(detailSize, event.key, detailMinSize, mainMinSize);
		if (nextSize === null) return;
		event.preventDefault();
		detailSize = nextSize;
	}
</script>

{#if !detailOpen || !detailPanel}
	<div class={cnfast("h-full min-h-0 min-w-0 flex-1 overflow-hidden", className)}>
		{@render children?.()}
	</div>
{:else}
	<div bind:this={frameElement} class={cnfast("relative flex h-full min-h-0 min-w-0 flex-1 overflow-hidden", className)}>
		<section class="h-full min-w-0 overflow-hidden max-xl:!basis-full" style={`flex: 0 0 ${mainSize}%;`}>
			{@render children?.()}
		</section>
		<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
		<div
			role="separator"
			aria-label="Resize details panel"
			aria-orientation="vertical"
			aria-valuemin={detailMinSize}
			aria-valuemax={100 - mainMinSize}
			aria-valuenow={Math.round(detailSize)}
			tabindex="0"
			class="group relative flex w-2 shrink-0 cursor-col-resize items-center justify-center bg-transparent outline-none focus-visible:bg-border/50 max-xl:hidden"
			onpointerdown={startResize}
			onkeydown={handleResizeKeydown}
		>
			<div class="h-full w-px bg-border/70 group-hover:bg-border"></div>
			<div class="absolute h-8 w-1 rounded-full bg-border opacity-70"></div>
		</div>
		<aside
			class="h-full min-w-0 overflow-hidden max-xl:!basis-full border-l bg-surface-1 max-xl:absolute max-xl:inset-0 max-xl:z-10 max-xl:shadow-xl"
			style={`flex: 0 0 ${detailSize}%;`}
		>
			{@render detailPanel?.()}
		</aside>
	</div>
{/if}
