<script lang="ts">
	import { ChevronRight } from "lucide-svelte";
	import { slide } from "svelte/transition";
	import { Button } from "@/components/ui/svelte";
	import { cnfast } from "@/lib/utils";
	import { nodeIdToString, type TreeNode, type TreeNodeId } from "@/lib/tree-nav";
	import { controlEase } from "./controlEase";
	import SidebarTreeNode from "./SidebarTreeNode.svelte";
	import {
		getResourceGroupVisual,
		getResourceKindVisual,
		type ResourceVisual,
	} from "./resourceVisuals";

	let {
		node,
		depth,
		selectedNode,
		revealKey = null,
		onReveal,
		selectedAncestorIds = new Set<string>(),
		expandedSections,
		onNodeSelect,
		onSectionToggle,
		getLazyChildren,
		customResourceSearch = "",
		onCustomResourceSearchChange,
	}: {
		node: TreeNode;
		depth: number;
		selectedNode: TreeNodeId | null;
		revealKey?: string | null;
		onReveal?: () => void;
		selectedAncestorIds?: ReadonlySet<string>;
		expandedSections: string[];
		onNodeSelect: (id: TreeNodeId) => void;
		onSectionToggle: (id: string) => void;
		getLazyChildren?: (node: TreeNode) => TreeNode[] | undefined;
		customResourceSearch?: string;
		onCustomResourceSearchChange?: (value: string) => void;
	} = $props();

	const id = $derived(nodeIdToString(node.id));
	const selectedKey = $derived(selectedNode ? nodeIdToString(selectedNode) : null);
	const isSelected = $derived(selectedKey === id);
	const isExpanded = $derived(expandedSections.includes(id));
	const hidesSelection = $derived(!isExpanded && selectedAncestorIds.has(id));
	// Svelte's element types do not list the ARIA 1.3 `aria-description` attribute yet, so it is spread.
	const selectionHint = $derived<Record<string, string>>(
		hidesSelection ? { "aria-description": "Contains the selected item" } : {},
	);
	const hasChildren = $derived(
		node.id.type === "namespace" || Boolean(node.children?.length),
	);
	const children = $derived(isExpanded ? (getLazyChildren?.(node) ?? node.children) : node.children);
	/** The Custom Resources section shows a search input above its kinds. */
	const hasSearch = $derived(node.id.type === "section" && node.id.section === "discovered");
	const isDisabled = $derived(node.disabled === true);
	const visual = $derived(getNodeVisual(node));
	const NodeIcon = $derived(visual.icon);
	const depthClass = $derived(
		depth === 0
			? "pl-2"
			: depth === 1
				? "pl-6"
				: depth === 2
					? "pl-10"
				: depth === 3
					? "pl-14"
					: "pl-[72px]",
	);

	let rowElement: HTMLDivElement | undefined;
	let highlighted = $state(false);
	let previousRevealKey: string | null = null;

	$effect(() => {
		const key = revealKey;
		if (key === null || key === previousRevealKey || !(isSelected || hidesSelection) || !rowElement) return;
		if (rowElement.getClientRects().length === 0) return;
		previousRevealKey = key;
		rowElement.scrollIntoView({ block: "nearest" });
		if (!window.matchMedia("(prefers-reduced-motion: reduce)").matches) highlighted = true;
		onReveal?.();
	});

	/** Kind nodes use their kind tone; everything else uses the group tone. */
	function getNodeVisual(item: TreeNode): ResourceVisual {
		if (item.id.type === "kind" && item.id.kind) {
			return getResourceKindVisual(item.id.kind);
		}
		if (item.id.type === "namespace") {
			return getResourceGroupVisual("Namespaces");
		}
		return getResourceGroupVisual(item.label);
	}

	function selectNode() {
		if (isDisabled || node.selectable === false) return;
		onNodeSelect(node.id);
	}

	/** Tailwind's `--default-transition-duration`, which the chevron beside the branch turns over. */
	const FOLD_MS = 150;
	/** The expansion the user last asked of this node, until its children have folded to it. */
	let toggledTo: boolean | null = null;

	function toggleSection() {
		toggledTo = !isExpanded;
		onSectionToggle(id);
	}

	/**
	 * The children fold open or closed only where the user toggled this node. "Collapse all", a
	 * reveal and a selection that opens the node show at once, as does reduced motion.
	 */
	function foldDuration(): number {
		if (toggledTo !== isExpanded) return 0;
		return window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : FOLD_MS;
	}

	/** The children's folding wrapper while it is mounted. */
	let branch = $state<HTMLElement>();

	// Reduced motion turned on while the children fold ends the fold, as it ends any transition.
	$effect(() => {
		const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
		const settle = () => {
			if (!reduced.matches) return;
			for (const animation of branch?.getAnimations() ?? []) animation.finish();
		};
		reduced.addEventListener("change", settle);
		return () => reduced.removeEventListener("change", settle);
	});

	/**
	 * Svelte reverses a running slide with that slide's options, so a change that shows at once,
	 * such as "Collapse all" while the children unfold, would still fold back over its time. That
	 * slide's animation starts right after this event, and is ended as it starts.
	 */
	function settleUntoggled(event: Event) {
		if (foldDuration() !== 0) return;
		const wrapper = event.currentTarget as HTMLElement;
		queueMicrotask(() => {
			for (const animation of wrapper.getAnimations()) animation.finish();
		});
	}

	function toggleNode(event: MouseEvent) {
		event.stopPropagation();
		if (hasChildren) toggleSection();
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === "Enter" || event.key === " ") {
			// A focused chevron button handles its own Enter and Space as a click.
			if (event.target !== event.currentTarget) return;
			event.preventDefault();
			selectNode();
			return;
		}
		if (!hasChildren) return;
		if (event.key === "ArrowRight" && !isExpanded) {
			event.preventDefault();
			toggleSection();
		} else if (event.key === "ArrowLeft" && isExpanded) {
			event.preventDefault();
			toggleSection();
		}
	}
</script>

<li>
	<div
		bind:this={rowElement}
		class={cnfast(
			"relative flex h-[26px] cursor-pointer select-none items-center gap-1 rounded-none text-xs text-sidebar-foreground/80 transition-colors hover:bg-surface-1 hover:text-sidebar-accent-foreground",
			isDisabled &&
				"cursor-default text-muted-foreground/70 hover:bg-transparent hover:text-muted-foreground/70",
			isSelected &&
				"bg-primary/10 text-sidebar-accent-foreground before:absolute before:bottom-0 before:left-0 before:top-0 before:w-0.5 before:rounded-r-sm before:bg-primary",
			depth === 0 && node.id.type !== "namespace" && "font-medium",
			highlighted && "animate-[sidebar-row-reveal_1s_ease-out]",
			depthClass,
		)}
		data-depth={depth}
		role="treeitem"
		tabindex={isDisabled ? -1 : 0}
		aria-selected={isSelected}
		aria-expanded={hasChildren ? isExpanded : undefined}
		aria-disabled={isDisabled || undefined}
		{...selectionHint}
		title={node.description}
		onclick={selectNode}
		onkeydown={handleKeydown}
		onanimationend={() => (highlighted = false)}
	>
		<Button
			type="button"
			variant="ghost"
			size="icon-xs"
			class={cnfast(
				"size-[18px] rounded-none p-0 text-muted-foreground hover:bg-transparent hover:text-foreground",
				!hasChildren && "invisible",
			)}
			aria-label={`${isExpanded ? "Collapse" : "Expand"} ${node.label}`}
			disabled={!hasChildren}
			onclick={toggleNode}
		>
			<ChevronRight
				class={cnfast("size-3 shrink-0 transition-transform", isExpanded && "rotate-90")}
				aria-hidden="true"
			/>
		</Button>
		<NodeIcon
			class={cnfast(
				"size-3.5 shrink-0",
				depth === 0 && "size-3",
				visual.className,
				isDisabled && "opacity-70",
			)}
			aria-hidden="true"
		/>
		<span class={cnfast("min-w-0 flex-1 truncate leading-none", hidesSelection && "text-primary")}
			>{node.label}</span
		>
	</div>
	{#if isExpanded && (hasSearch || (hasChildren && children))}
		<div
			bind:this={branch}
			data-slot="sidebar-branch"
			transition:slide={{ duration: foldDuration(), easing: controlEase }}
			onintrostart={settleUntoggled}
			onoutrostart={settleUntoggled}
			onintroend={() => (toggledTo = null)}
			onoutroend={() => (toggledTo = null)}
		>
			{#if hasSearch}
				<div class="px-2 py-1 pl-6">
					<input
						class="h-7 w-full rounded-sm border border-border bg-background px-2 text-xs text-foreground placeholder:text-muted-foreground"
						value={customResourceSearch}
						placeholder="Search custom resources"
						aria-label="Search custom resources"
						oninput={(event) => onCustomResourceSearchChange?.(event.currentTarget.value)}
					/>
				</div>
			{/if}
			{#if hasChildren && children}
				<ul class="m-0 list-none p-0" role="group">
					{#each children as child (nodeIdToString(child.id))}
						<SidebarTreeNode
							node={child}
							depth={depth + 1}
							{selectedNode}
							{revealKey}
							{onReveal}
							{selectedAncestorIds}
							{expandedSections}
							{onNodeSelect}
							{onSectionToggle}
							{getLazyChildren}
							{customResourceSearch}
							{onCustomResourceSearchChange}
						/>
					{/each}
				</ul>
			{/if}
		</div>
	{/if}
</li>
