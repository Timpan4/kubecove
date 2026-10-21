<script lang="ts" generics="Item">
	import { createVirtualizer, defaultRangeExtractor } from "@tanstack/svelte-virtual";
	import { onMount, tick, untrack, type Snippet } from "svelte";

	let { items, itemKey, selectedKey = "", gridClass = "", estimateSize, minWidth, children }: {
		items: Item[];
		itemKey: (item: Item) => string;
		selectedKey?: string;
		gridClass?: string;
		estimateSize: number;
		minWidth?: string;
		children: Snippet<[Item]>;
	} = $props();

	let host: HTMLDivElement;
	let probe: HTMLDivElement;
	let width = $state(0);
	let rem = $state(16);
	let columns = $state(1);
	let template = $state("");
	let gap = $state(0);
	let scrollElement = $state<HTMLElement | null>(null);
	let margin = $state(0);
	let focusedKey = $state<string | null>(null);
	let restoringFocus = false;
	let previousSelection = "";
	// Enough probe cells to fill the viewport at the cards' existing 18.75rem minimum.
	const probeCount = $derived(gridClass ? Math.min(items.length, Math.max(2, Math.ceil(width / (18.75 * rem)))) : 1);
	const rowCount = $derived(Math.ceil(items.length / columns));
	const virtualizer = createVirtualizer<HTMLElement, HTMLDivElement>({
		count: 0,
		getScrollElement: () => scrollElement,
		estimateSize: () => estimateSize,
	});

	$effect(() => {
		const count = rowCount;
		const rowColumns = columns;
		const rowItems = items;
		const focusedIndex = focusedKey === null ? -1 : rowItems.findIndex((item) => itemKey(item) === focusedKey);
		const pinned = focusedIndex < 0 ? null : Math.floor(focusedIndex / rowColumns);
		const scrollMargin = margin;
		const rowGap = gap;
		const element = scrollElement;
		const cardGrid = Boolean(gridClass);
		untrack(() => $virtualizer.setOptions({
			count, getScrollElement: () => element, scrollMargin, gap: rowGap,
			estimateSize: () => estimateSize,
			getItemKey: (index) => cardGrid ? `${rowColumns}:${index}` : itemKey(rowItems[index]),
			rangeExtractor: (range) => {
				const indexes = defaultRangeExtractor(range);
				if (pinned === null) return indexes;
				// Keep focus and its Tab neighbors mounted even when scrolled away.
				for (const index of [pinned - 1, pinned, pinned + 1]) {
					if (index >= 0 && index < count && !indexes.includes(index)) indexes.push(index);
				}
				return indexes.sort((a, b) => a - b);
			},
		}));
	});

	$effect(() => {
		const key = selectedKey;
		const rowColumns = columns;
		if (!key) {
			previousSelection = "";
			return;
		}
		if (!scrollElement) return;
		const index = items.findIndex((item) => itemKey(item) === key);
		if (index < 0) return;
		const selection = key;
		if (selection === previousSelection) return;
		previousSelection = selection;
		void tick().then(() => $virtualizer.scrollToIndex(Math.floor(index / rowColumns), { align: "auto" }));
	});

	onMount(() => {
		// overflow-x-auto tables also compute to overflow-y:auto, but do not
		// constrain height. Use the app's vertical viewport above those wrappers.
		let ancestor = host.closest<HTMLElement>(".overflow-y-auto, .overflow-auto");
		if (!ancestor) {
			ancestor = host.parentElement;
			while (ancestor && !/(auto|scroll)/.test(getComputedStyle(ancestor).overflowY)) ancestor = ancestor.parentElement;
		}
		scrollElement = ancestor ?? document.documentElement;
		const measure = () => {
			rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
			measureLayout();
			if (scrollElement) margin = host.getBoundingClientRect().top - scrollElement.getBoundingClientRect().top - scrollElement.clientTop + scrollElement.scrollTop;
		};
		const observer = new ResizeObserver(measure);
		// Observe the layout path and preceding siblings, whose height can move
		// the list without resizing either the list or its scroll viewport.
		const layoutObserver = new MutationObserver(() => {
			observeLayout();
			measure();
		});
		function observeLayout() {
			observer.disconnect();
			layoutObserver.disconnect();
			observer.observe(host);
			let node: HTMLElement | null = host;
			while (node && node !== scrollElement) {
				for (let sibling = node.previousElementSibling; sibling; sibling = sibling.previousElementSibling) observer.observe(sibling);
				node = node.parentElement;
				if (node) {
					observer.observe(node);
					layoutObserver.observe(node, { childList: true });
				}
			}
		}
		observeLayout();
		measure();
		return () => {
			observer.disconnect();
			layoutObserver.disconnect();
		};
	});

	$effect(() => {
		void [probeCount, gridClass];
		void tick().then(() => {
			if (!probe) return;
			measureLayout();
		});
	});

	function measureLayout() {
		const style = getComputedStyle(probe);
		const tracks = style.gridTemplateColumns.split(" ").filter((track) => Number.parseFloat(track) > 0);
		const nextColumns = gridClass ? Math.max(1, tracks.length) : 1;
		if (columns !== nextColumns) preserveFocus();
		columns = nextColumns;
		template = gridClass ? style.gridTemplateColumns : "";
		gap = Number.parseFloat(style.rowGap) || 0;
	}

	$effect.pre(() => {
		void items;
		untrack(preserveFocus);
	});

	function preserveFocus() {
		const active = document.activeElement;
		const item = active instanceof Element ? active.closest<HTMLElement>("[data-virtual-item]") : null;
		if (!item || !host?.contains(item)) return;
		const key = item.dataset.virtualItem;
		const index = [...item.querySelectorAll("button, a, input, [tabindex]")].indexOf(active!);
		restoringFocus = true;
		void tick().then(() => {
			if (!active?.isConnected) {
				const target = [...host.querySelectorAll<HTMLElement>("[data-virtual-item]")].find((node) => node.dataset.virtualItem === key);
				target?.querySelectorAll<HTMLElement>("button, a, input, [tabindex]")[index]?.focus({ preventScroll: true });
			}
			restoringFocus = false;
		});
	}

	function measureRow(node: HTMLDivElement) {
		$virtualizer.measureElement(node);
	}
	function trackFocus(event: FocusEvent) {
		const target = event.target;
		const item = target instanceof Element ? target.closest<HTMLElement>("[data-virtual-item]") : null;
		focusedKey = item?.dataset.virtualItem ?? null;
	}
</script>

<div bind:this={host} bind:clientWidth={width} class="relative min-w-0" style:min-width={minWidth}
	onfocusin={trackFocus} onfocusout={(event) => { if (!restoringFocus && (!(event.relatedTarget instanceof Node) || !host.contains(event.relatedTarget))) focusedKey = null; }}>
	<div bind:this={probe} class={gridClass} aria-hidden="true" style="position:absolute;inset:0 0 auto;visibility:hidden;pointer-events:none;height:0;overflow:hidden;">
		{#each Array.from({ length: probeCount }) as _}<div></div>{/each}
	</div>
	<div class="relative" style:height={`${$virtualizer.getTotalSize()}px`}>
		{#each $virtualizer.getVirtualItems() as row (row.key)}
			<div data-virtual-row data-index={row.index} use:measureRow class={gridClass}
				style:position="absolute" style:top="0" style:left="0" style:width="100%"
				style:grid-template-columns={template || undefined} style:transform={`translateY(${row.start - margin}px)`}>
				{#each items.slice(row.index * columns, (row.index + 1) * columns) as item (itemKey(item))}
					<div class="min-w-0" data-virtual-item={itemKey(item)}>{@render children(item)}</div>
				{/each}
			</div>
		{/each}
	</div>
</div>
