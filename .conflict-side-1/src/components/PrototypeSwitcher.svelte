<script lang="ts">
	import { ChevronLeft, ChevronRight } from "lucide-svelte";
	import { Button } from "@/components/ui/svelte";

	export interface PrototypeVariantOption {
		value: string;
		label: string;
		description: string;
	}

	let {
		label,
		variants,
		value,
		onChange,
	}: {
		label: string;
		variants: PrototypeVariantOption[];
		value: string;
		onChange: (value: string) => void;
	} = $props();

	const visible = import.meta.env.DEV;
	const activeIndex = $derived(Math.max(0, variants.findIndex((variant) => variant.value === value)));
	const activeVariant = $derived(variants[activeIndex] ?? variants[0]);

	function cycle(offset: number) {
		if (variants.length === 0) return;
		const nextIndex = (activeIndex + offset + variants.length) % variants.length;
		onChange(variants[nextIndex].value);
	}

	function handleKeydown(event: KeyboardEvent) {
		if (!visible || (event.key !== "ArrowLeft" && event.key !== "ArrowRight")) return;
		const target = event.target;
		if (
			target instanceof HTMLInputElement ||
			target instanceof HTMLTextAreaElement ||
			(target instanceof HTMLElement && target.isContentEditable)
		) return;
		event.preventDefault();
		cycle(event.key === "ArrowLeft" ? -1 : 1);
	}
</script>

<svelte:window onkeydown={handleKeydown} />

{#if visible && activeVariant}
	<div class="pointer-events-none fixed inset-x-0 bottom-5 z-[100] flex justify-center px-4">
		<div class="pointer-events-auto flex max-w-full items-center gap-1 rounded-full border bg-popover/95 p-1 shadow-xl backdrop-blur-md">
			<Button type="button" size="icon-sm" variant="ghost" class="rounded-full" aria-label="Previous {label} prototype" onclick={() => cycle(-1)}><ChevronLeft /></Button>
			<div class="min-w-0 px-2 text-center">
				<div class="text-[0.5625rem] font-semibold uppercase tracking-[0.16em] text-muted-foreground">Prototype · {label}</div>
				<div class="truncate text-xs font-medium">{activeVariant.label} <span class="font-normal text-muted-foreground">— {activeVariant.description}</span></div>
			</div>
			<Button type="button" size="icon-sm" variant="ghost" class="rounded-full" aria-label="Next {label} prototype" onclick={() => cycle(1)}><ChevronRight /></Button>
		</div>
	</div>
{/if}
