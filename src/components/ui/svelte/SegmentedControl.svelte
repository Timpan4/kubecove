<script lang="ts" generics="T extends string">
	import type { Snippet } from "svelte";
	import { cnfast } from "@/lib/utils";

	type Option = {
		value: T;
		label: string;
		icon?: Snippet;
	};

	let {
		value,
		options,
		onChange,
		ariaLabel,
		size = "sm",
		class: className = "",
	}: {
		value: T;
		options: Option[];
		onChange: (value: T) => void;
		ariaLabel: string;
		size?: "sm" | "md";
		class?: string;
	} = $props();

	let group: HTMLDivElement | undefined = $state();
	/** The active option's box inside the group, which the pill covers. */
	let pill: { left: number; top: number; width: number; height: number } | undefined = $state();
	/**
	 * The pill slides only to a newly chosen option. A layout change moves it at once. At rest the
	 * active option paints its own background, because `offsetWidth` rounds the option's fractional
	 * width and a pill measured from it would miss the option's edge by a pixel.
	 */
	let sliding = $state(false);
	let shownValue: T | undefined;

	function place() {
		const active = group?.querySelector<HTMLElement>("[aria-checked='true']");
		pill = active
			? {
					left: active.offsetLeft,
					top: active.offsetTop,
					width: active.offsetWidth,
					height: active.offsetHeight,
				}
			: undefined;
	}

	$effect(() => {
		if (!group) return;
		sliding = shownValue !== undefined && shownValue !== value;
		shownValue = value;
		place();
		const observer = new ResizeObserver(place);
		observer.observe(group);
		return () => observer.disconnect();
	});

	/**
	 * Ends the slide once none of the pill's transitions is left. A choice made mid-slide cancels
	 * the running transitions and starts others, which still need the transition class.
	 */
	function settle(event: TransitionEvent & { currentTarget: HTMLSpanElement }) {
		if (event.currentTarget.getAnimations().length === 0) sliding = false;
	}
</script>

<div
	bind:this={group}
	role="radiogroup"
	aria-label={ariaLabel}
	class={cnfast(
		"relative inline-flex items-center rounded-lg bg-surface-0 p-0.5 ring-1 ring-border/60",
		className,
	)}
>
	{#if pill}
		<span
			data-slot="segmented-control-pill"
			aria-hidden="true"
			class={cnfast(
				"pointer-events-none absolute rounded-md bg-surface-1 shadow-sm",
				sliding ? "transition-[left,top,width,height] duration-150" : "invisible",
			)}
			style:left="{pill.left}px"
			style:top="{pill.top}px"
			style:width="{pill.width}px"
			style:height="{pill.height}px"
			ontransitionend={settle}
			ontransitioncancel={settle}
		></span>
	{/if}
	{#each options as option (option.value)}
		{@const active = option.value === value}
		<button
			type="button"
			role="radio"
			aria-checked={active}
			onclick={() => onChange(option.value)}
			class={cnfast(
				"relative inline-flex cursor-pointer items-center gap-1.5 rounded-md font-medium transition-[color] duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40 [&_svg]:pointer-events-none [&_svg]:size-3.5",
				size === "md" ? "h-7 px-2.5 text-xs" : "h-6 px-2 text-xs",
				active ? "text-foreground" : "text-muted-foreground hover:text-foreground",
				active && !sliding && "bg-surface-1 shadow-sm",
			)}
		>
			{#if option.icon}
				{@render option.icon()}
			{/if}
			<span>{option.label}</span>
		</button>
	{/each}
</div>
