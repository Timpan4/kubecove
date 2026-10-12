<script lang="ts">
	import { untrack } from "svelte";
	import { ChevronsDownUp } from "lucide-svelte";
	import {
		Badge,
		Button,
		Sheet,
		SheetContent,
		SheetTitle,
		Sidebar,
		SidebarContent,
	} from "@/components/ui/svelte";
	import { type SavedWorkspace, workspaceScopeContexts } from "@/lib/workspace-model";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import { nodeIdToString, type TreeNodeId } from "@/lib/tree-nav";
	import SidebarTree from "./SidebarTree.svelte";
	import { toggleCompactSidebarSection } from "./workspaceShellModel";

	let {
		workspace,
		workspaceReadContext,
		selectedNode,
		sidebarNavigationRevision,
		expandedSections = $bindable(),
		navigationOpen = $bindable(),
		onNodeSelect,
	}: {
		workspace: SavedWorkspace;
		workspaceReadContext: WorkspaceReadContext;
		selectedNode: TreeNodeId | null;
		sidebarNavigationRevision: number;
		expandedSections: string[];
		navigationOpen: boolean;
		onNodeSelect: (id: TreeNodeId) => void;
	} = $props();

	let compactScrollTop = $state(0);
	let compactRevealKey = $state<string | null>(null);
	let compactSelectedKey = $state<string | null>(untrack(() => selectedNode ? `${nodeIdToString(selectedNode)}:${sidebarNavigationRevision}` : null));
	const contextCount = $derived(workspaceScopeContexts(workspace.scope).length);

	function toggleSection(id: string) {
		expandedSections = expandedSections.includes(id)
			? expandedSections.filter((item) => item !== id)
			: [...expandedSections, id];
	}

	function toggleCompactSection(id: string) {
		expandedSections = toggleCompactSidebarSection(expandedSections, id);
	}
</script>

{#snippet header(paddingClass: string)}
	<div class="border-b px-3 py-3 {paddingClass}">
		<div class="flex items-center justify-between gap-2">
			<div class="min-w-0">
				<div class="truncate text-sm font-semibold">{workspace.name}</div>
				<div class="truncate text-xs text-muted-foreground">
					{workspace.scope.clusterContext}
				</div>
			</div>
			<div class="flex shrink-0 items-center gap-1">
				<Badge variant="outline" class="tabular-nums">{contextCount}</Badge>
				<Button
					type="button"
					variant="ghost"
					size="icon-sm"
					aria-label="Collapse all sidebar sections"
					title="Collapse all"
					onclick={() => (expandedSections = [])}
				>
					<ChevronsDownUp aria-hidden="true" />
				</Button>
			</div>
		</div>
	</div>
{/snippet}

<Sidebar
	class="hidden w-[260px] shrink-0 flex-col border-r bg-surface-1 xl:flex xl:overflow-hidden xl:rounded-xl xl:border xl:border-sidebar-border xl:shadow-lg"
>
	{@render header("")}
	<SidebarContent>
		<SidebarTree
			{workspaceReadContext}
			{selectedNode}
			{sidebarNavigationRevision}
			{expandedSections}
			{onNodeSelect}
			onSectionToggle={toggleSection}
		/>
	</SidebarContent>
</Sidebar>

{#if navigationOpen}
	<Sheet open onOpenChange={(open: boolean) => (navigationOpen = open)}>
		<SheetContent side="left" class="w-[280px] max-w-[85vw] p-0 xl:hidden">
			<SheetTitle class="sr-only">Workspace navigation</SheetTitle>
			{@render header("pr-12")}
			<SidebarContent>
				<SidebarTree
					{workspaceReadContext}
					{selectedNode}
					{sidebarNavigationRevision}
					{expandedSections}
					{onNodeSelect}
					onSectionToggle={toggleCompactSection}
					bind:scrollTop={compactScrollTop}
					bind:previousSelectedKey={compactSelectedKey}
					bind:revealKey={compactRevealKey}
				/>
			</SidebarContent>
		</SheetContent>
	</Sheet>
{/if}
