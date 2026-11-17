<script lang="ts" module>
	/** The trigger that last carried each tab list's line, which the next one's line slides from. */
	const lined = new WeakMap<Element, HTMLElement>();

	/** Where each list's lined trigger lay when last measured, as `linePlace` gives it. */
	const placed = new WeakMap<HTMLElement, string>();

	const LINE_PROPERTIES = ["--tabs-line-x", "--tabs-line-y", "--tabs-line-w", "--tabs-line-h", "--tabs-line-duration"];

	/** The numbers of a computed `translate` or `scale`, which reads `none` where nothing is set. */
	function numbers(value: string): number[] {
		return value === "none" ? [] : value.split(" ").map(Number.parseFloat);
	}

	/** The trigger's padding box, which its line spans, as the line is positioned inside it. */
	function paddingBox(trigger: HTMLElement) {
		const box = trigger.getBoundingClientRect();
		const style = getComputedStyle(trigger);
		const [top, right, bottom, left] = [style.borderTopWidth, style.borderRightWidth, style.borderBottomWidth, style.borderLeftWidth].map(Number.parseFloat);
		return {
			left: box.left + left,
			top: box.top + top,
			right: box.right - right,
			bottom: box.bottom - bottom,
			width: box.width - left - right,
			height: box.height - top - bottom,
		};
	}

	/**
	 * Carries the list's line to `trigger`, which became active. Its own line starts where the last
	 * active trigger's is shown, which is on its way home if that slide still runs, and moves home
	 * over the `after:` transition. The offset is set with the transition held at 0s and released
	 * after a style flush, so only the way home animates.
	 */
	function carryLine(trigger: HTMLElement) {
		const list = trigger.closest("[data-slot='tabs-list']");
		if (!list) return;
		const last = lined.get(list);
		lined.set(list, trigger);
		placed.set(trigger, linePlace(trigger));
		if (!last || last === trigger || !last.isConnected) return;
		// Under reduced motion the line shows under the chosen trigger at once.
		if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
		const [from, to] = [paddingBox(last), paddingBox(trigger)];
		if (to.width === 0 || to.height === 0) return;
		// The line spans the trigger's width under it, or its height beside it in a vertical list.
		const vertical = list.getAttribute("data-orientation") === "vertical";
		const shown = getComputedStyle(last, "::after");
		const [x = 0, y = 0] = numbers(shown.translate);
		const [w = 1, h = w] = numbers(shown.scale);
		const offset = vertical
			? [from.right - to.right + x, from.top - to.top + y, 1, (from.height * h) / to.height]
			: [from.left - to.left + x, from.bottom - to.bottom + y, (from.width * w) / to.width, 1];
		trigger.style.setProperty("--tabs-line-x", `${offset[0]}px`);
		trigger.style.setProperty("--tabs-line-y", `${offset[1]}px`);
		trigger.style.setProperty("--tabs-line-w", String(offset[2]));
		trigger.style.setProperty("--tabs-line-h", String(offset[3]));
		trigger.style.setProperty("--tabs-line-duration", "0s");
		void getComputedStyle(trigger, "::after").translate;
		for (const property of LINE_PROPERTIES) trigger.style.removeProperty(property);
	}

	/**
	 * Where the trigger's line lies in its list: its start and length along the list, and its row
	 * or column across it. A slide's offsets go stale only when this changes. The row is measured
	 * from the list's first trigger, so a change that moves every trigger alike across the list,
	 * such as the list growing taller as a tab panel opens, leaves it alone.
	 */
	function linePlace(trigger: HTMLElement): string {
		const list = trigger.closest("[data-slot='tabs-list']");
		const first = list?.querySelector<HTMLElement>("[data-slot='tabs-trigger']");
		const box = paddingBox(trigger);
		const origin = list?.getBoundingClientRect() ?? { left: 0, top: 0 };
		const row = first ? paddingBox(first) : box;
		return list?.getAttribute("data-orientation") === "vertical"
			? `${box.top - origin.top} ${box.height} ${box.left - row.left}`
			: `${box.left - origin.left} ${box.width} ${box.top - row.top}`;
	}

	/**
	 * Ends the list's slide at once if a layout change moved its active trigger's line. Every
	 * trigger's resize calls this, because a sibling that grows can move the active trigger without
	 * resizing it or the list.
	 */
	function settleList(list: Element) {
		const active = lined.get(list);
		if (!active?.isConnected) return;
		const now = linePlace(active);
		if (placed.get(active) !== now && active.getAttribute("data-state") === "active") endSlide(active);
		placed.set(active, now);
	}

	/** Moves the trigger's line home at once, ending its slide. */
	function endSlide(trigger: HTMLElement) {
		// A pseudo-element's animations are listed only with the subtree.
		for (const animation of trigger.getAnimations({ subtree: true })) {
			const effect = animation.effect;
			if (effect instanceof KeyframeEffect && effect.target === trigger && effect.pseudoElement === "::after") animation.finish();
		}
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
		const list = element.closest("[data-slot='tabs-list']");
		if (list && active()) {
			lined.set(list, element);
			placed.set(element, linePlace(element));
		}
		const observer = new MutationObserver(() => {
			if (active()) carryLine(element);
		});
		observer.observe(element, { attributes: true, attributeFilter: ["data-state"] });
		// A resize or a wrap can move the active trigger in its list, and the slide's offsets were
		// measured before it. So such a move ends the slide and the line shows under it at once.
		if (!list) return () => observer.disconnect();
		const resized = new ResizeObserver(() => settleList(list));
		resized.observe(list, { box: "border-box" });
		resized.observe(element, { box: "border-box" });
		return () => {
			observer.disconnect();
			resized.disconnect();
		};
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
