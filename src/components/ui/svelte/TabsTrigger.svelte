<script lang="ts" module>
	/** The trigger that last carried each tab list's line, which the next one's line slides from. */
	const lined = new WeakMap<Element, HTMLElement>();

	const LINE_PROPERTIES = ["--tabs-line-x", "--tabs-line-y", "--tabs-line-w", "--tabs-line-h", "--tabs-line-duration"];

	/**
	 * Carries the list's line to `trigger`, which became active. Its own line starts where the last
	 * active trigger's was and moves home over the `after:` transition. The offset is set with the
	 * transition held at 0s and released after a style flush, so only the way home animates.
	 */
	function carryLine(trigger: HTMLElement) {
		const list = trigger.closest("[data-slot='tabs-list']");
		if (!list) return;
		const last = lined.get(list);
		lined.set(list, trigger);
		if (!last || last === trigger || !last.isConnected) return;
		const [from, to] = [last.getBoundingClientRect(), trigger.getBoundingClientRect()];
		if (to.width === 0 || to.height === 0) return;
		// The line spans the trigger's width under it, or its height beside it in a vertical list.
		const vertical = list.getAttribute("data-orientation") === "vertical";
		const offset = vertical
			? [from.right - to.right, from.top - to.top, 1, from.height / to.height]
			: [from.left - to.left, from.bottom - to.bottom, from.width / to.width, 1];
		trigger.style.setProperty("--tabs-line-x", `${offset[0]}px`);
		trigger.style.setProperty("--tabs-line-y", `${offset[1]}px`);
		trigger.style.setProperty("--tabs-line-w", String(offset[2]));
		trigger.style.setProperty("--tabs-line-h", String(offset[3]));
		trigger.style.setProperty("--tabs-line-duration", "0s");
		void getComputedStyle(trigger, "::after").translate;
		for (const property of LINE_PROPERTIES) trigger.style.removeProperty(property);
	}
</script>

<script lang="ts">
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		value,
		children,
		...rest
	}: UiProps & { value: string } = $props();

	let trigger: HTMLElement | null = $state(null);

	$effect(() => {
		const element = trigger;
		if (!element) return;
		const active = () => element.getAttribute("data-state") === "active";
		if (active()) {
			const list = element.closest("[data-slot='tabs-list']");
			if (list) lined.set(list, element);
		}
		const observer = new MutationObserver(() => {
			if (active()) carryLine(element);
		});
		observer.observe(element, { attributes: true, attributeFilter: ["data-state"] });
		return () => observer.disconnect();
	});
</script>

<TabsPrimitive.Trigger
	bind:ref={trigger}
	data-slot="tabs-trigger"
	{value}
	class={cnfast(
		"relative inline-flex h-[calc(100%-1px)] flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-md border border-transparent px-1.5 py-0.5 text-xs font-medium whitespace-nowrap text-foreground/60 transition-[color,background-color,border-color,box-shadow] group-data-[orientation=vertical]/tabs:w-full group-data-[orientation=vertical]/tabs:justify-start group-data-[orientation=vertical]/tabs:py-[calc(--spacing(1.25))] hover:text-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1 focus-visible:outline-ring disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 has-data-[icon=inline-end]:pr-1 has-data-[icon=inline-start]:pl-1 dark:text-muted-foreground dark:hover:text-foreground [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5 group-data-[variant=line]/tabs-list:bg-transparent group-data-[variant=line]/tabs-list:data-[state=active]:bg-transparent dark:group-data-[variant=line]/tabs-list:data-[state=active]:border-transparent dark:group-data-[variant=line]/tabs-list:data-[state=active]:bg-transparent data-[state=active]:bg-background data-[state=active]:text-foreground dark:data-[state=active]:border-input dark:data-[state=active]:bg-input/30 dark:data-[state=active]:text-foreground after:absolute after:origin-top-left after:bg-foreground after:opacity-0 after:transition-[translate,scale] after:duration-[var(--tabs-line-duration,150ms)] after:[scale:var(--tabs-line-w,1)_var(--tabs-line-h,1)] after:[translate:var(--tabs-line-x,0px)_var(--tabs-line-y,0px)] group-data-[orientation=horizontal]/tabs:after:inset-x-0 group-data-[orientation=horizontal]/tabs:after:bottom-[-5px] group-data-[orientation=horizontal]/tabs:after:h-0.5 group-data-[orientation=vertical]/tabs:after:inset-y-0 group-data-[orientation=vertical]/tabs:after:-right-1 group-data-[orientation=vertical]/tabs:after:w-0.5 group-data-[variant=line]/tabs-list:data-[state=active]:after:opacity-100",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</TabsPrimitive.Trigger>
