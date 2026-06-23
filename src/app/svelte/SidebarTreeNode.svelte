<script lang="ts">
	import { ChevronRight } from "lucide-svelte";
	import { cn } from "@/lib/utils";
	import { nodeIdToString, type TreeNode, type TreeNodeId } from "@/lib/tree-nav";
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
		expandedSections,
		onNodeSelect,
		onSectionToggle,
		getLazyChildren,
	}: {
		node: TreeNode;
		depth: number;
		selectedNode: TreeNodeId | null;
		expandedSections: string[];
		onNodeSelect: (id: TreeNodeId) => void;
		onSectionToggle: (id: string) => void;
		getLazyChildren?: (node: TreeNode) => TreeNode[] | undefined;
	} = $props();

	const id = $derived(nodeIdToString(node.id));
	const isSelected = $derived(
		selectedNode !== null && nodeIdToString(selectedNode) === id,
	);
	const isExpanded = $derived(expandedSections.includes(id));
	const hasChildren = $derived(
		node.id.type === "namespace" || Boolean(node.children?.length),
	);
	const children = $derived(isExpanded ? (getLazyChildren?.(node) ?? node.children) : node.children);
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

	/** Mirrors React's `getNodeVisual` in SidebarTree.tsx — kind → kind tone, else group tone. */
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
		if (isDisabled) return;
		onNodeSelect(node.id);
		if (hasChildren) onSectionToggle(id);
	}

	function toggleNode(event: MouseEvent) {
		event.stopPropagation();
		if (!isDisabled) onNodeSelect(node.id);
		if (hasChildren) onSectionToggle(id);
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === "Enter" || event.key === " ") {
			event.preventDefault();
			selectNode();
			return;
		}
		if (!hasChildren) return;
		if (event.key === "ArrowRight" && !isExpanded) {
			event.preventDefault();
			onSectionToggle(id);
		} else if (event.key === "ArrowLeft" && isExpanded) {
			event.preventDefault();
			onSectionToggle(id);
		}
	}
</script>

<li>
	<div
		class={cn(
			"relative flex h-[26px] cursor-pointer select-none items-center gap-1 rounded-none text-[0.8125rem] text-sidebar-foreground/80 transition-colors hover:bg-surface-1 hover:text-sidebar-accent-foreground",
			isDisabled &&
				"cursor-default text-muted-foreground/70 hover:bg-transparent hover:text-muted-foreground/70",
			isSelected &&
				"bg-primary/10 text-sidebar-accent-foreground before:absolute before:bottom-0 before:left-0 before:top-0 before:w-0.5 before:rounded-r-sm before:bg-primary",
			depth === 0 &&
				"text-[0.6875rem] font-bold uppercase tracking-wide text-muted-foreground/70 hover:bg-transparent hover:text-foreground/70",
			depthClass,
		)}
		data-depth={depth}
		role="treeitem"
		tabindex={isDisabled ? -1 : 0}
		aria-selected={isSelected}
		aria-expanded={hasChildren ? isExpanded : undefined}
		aria-disabled={isDisabled || undefined}
		title={node.description}
		onclick={selectNode}
		onkeydown={handleKeydown}
	>
		<button
			type="button"
			class={cn(
				"flex size-[18px] shrink-0 cursor-pointer items-center justify-center border-0 bg-transparent p-0 text-muted-foreground transition-colors hover:text-foreground",
				!hasChildren && "invisible",
			)}
			aria-label={`${isExpanded ? "Collapse" : "Expand"} ${node.label}`}
			disabled={!hasChildren}
			onclick={toggleNode}
		>
			<ChevronRight
				class={cn("size-3 shrink-0 transition-transform", isExpanded && "rotate-90")}
				aria-hidden="true"
			/>
		</button>
		<NodeIcon
			class={cn(
				"size-3.5 shrink-0",
				depth === 0 && "size-3",
				visual.className,
				isDisabled && "opacity-70",
			)}
			aria-hidden="true"
		/>
		<span class="min-w-0 flex-1 truncate leading-none">{node.label}</span>
	</div>
	{#if hasChildren && isExpanded && children}
		<ul class="m-0 list-none p-0" role="group">
			{#each children as child (nodeIdToString(child.id))}
				<SidebarTreeNode
					node={child}
					depth={depth + 1}
					{selectedNode}
					{expandedSections}
					{onNodeSelect}
					{onSectionToggle}
					{getLazyChildren}
				/>
			{/each}
		</ul>
	{/if}
</li>
