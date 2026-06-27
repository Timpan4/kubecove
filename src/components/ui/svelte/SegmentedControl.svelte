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
</script>

<div
	role="radiogroup"
	aria-label={ariaLabel}
	class={cnfast(
		"inline-flex items-center rounded-lg bg-surface-0 p-0.5 ring-1 ring-border/60",
		className,
	)}
>
	{#each options as option (option.value)}
		{@const active = option.value === value}
		<button
			type="button"
			role="radio"
			aria-checked={active}
			onclick={() => onChange(option.value)}
			class={cnfast(
				"inline-flex cursor-pointer items-center gap-1.5 rounded-md font-medium transition-all duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40 [&_svg]:pointer-events-none [&_svg]:size-3.5",
				size === "md" ? "h-7 px-2.5 text-xs" : "h-6 px-2 text-xs",
				active
					? "bg-surface-1 text-foreground shadow-sm"
					: "text-muted-foreground hover:text-foreground",
			)}
		>
			{#if option.icon}
				{@render option.icon()}
			{/if}
			<span>{option.label}</span>
		</button>
	{/each}
</div>
