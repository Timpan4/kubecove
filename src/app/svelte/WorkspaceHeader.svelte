<script lang="ts">
	import { ChevronRight, FolderOpen, Menu, Search, Settings } from "lucide-svelte";
	import ClusterSelector from "@/components/ClusterSelector.svelte";
	import { Button } from "@/components/ui/svelte";
	import RemoteHostScope from "@/features/remote-connection/RemoteHostScope.svelte";
	import ActiveLiveSessionsButton from "./ActiveLiveSessionsButton.svelte";
	import UpdateStatusButton from "./UpdateStatusButton.svelte";

	const IS_MAC = /Mac/i.test(globalThis.navigator?.userAgent ?? "");
	const SEARCH_SHORTCUT_HINT = IS_MAC ? "⌘K" : "Ctrl K";

	let {
		clusterContext,
		locationLabel,
		focusApplicationName,
		onOpenNavigation,
		onClusterChange,
		onOpenPortForwards,
		onOpenLauncher,
		onOpenSettings,
		onOpenSearch,
	}: {
		clusterContext: string;
		locationLabel: string;
		focusApplicationName?: string;
		onOpenNavigation: () => void;
		onClusterChange: (clusterContext: string) => void;
		onOpenPortForwards: () => void;
		onOpenLauncher: () => void;
		onOpenSettings: () => void;
		onOpenSearch: () => void;
	} = $props();
</script>

{#snippet location()}
	<span class="truncate whitespace-nowrap text-xs font-medium text-muted-foreground">
		{locationLabel}
	</span>
	{#if focusApplicationName}
		<ChevronRight class="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
		<strong class="truncate text-xs font-semibold">{focusApplicationName}</strong>
	{/if}
{/snippet}

<header
	class="flex h-12 shrink-0 items-center gap-2 border-b bg-sidebar px-2 sm:gap-4 sm:px-4 [-webkit-app-region:drag]"
>
	<Button
		type="button"
		variant="ghost"
		size="icon"
		class="shrink-0 xl:hidden [-webkit-app-region:no-drag]"
		aria-label="Open workspace navigation"
		onclick={onOpenNavigation}
	>
		<Menu />
	</Button>
	<nav
		class="hidden min-w-0 flex-1 items-center gap-2 [-webkit-app-region:no-drag] md:flex"
		aria-label="Workspace location"
	>
		<ClusterSelector value={clusterContext} {onClusterChange} />
		<ChevronRight class="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
		{@render location()}
	</nav>
	<nav
		class="flex min-w-0 flex-1 items-center gap-1.5 md:hidden"
		aria-label="Workspace location"
	>
		{@render location()}
	</nav>
	<div class="flex min-w-0 shrink-0 items-center justify-end gap-1 [-webkit-app-region:no-drag] md:flex-1">
		<RemoteHostScope />
		<ActiveLiveSessionsButton onOpenManager={onOpenPortForwards} />
		<UpdateStatusButton />
		<Button type="button" variant="ghost" size="icon" aria-label="Open workspaces" onclick={onOpenLauncher}>
			<FolderOpen />
		</Button>
		<Button type="button" variant="ghost" size="icon" aria-label="Open settings" onclick={onOpenSettings}>
			<Settings />
		</Button>
		<Button
			type="button"
			variant="outline"
			size="sm"
			class="hidden gap-2 whitespace-nowrap bg-surface-1 px-3 text-xs font-normal text-muted-foreground shadow-xs [-webkit-app-region:no-drag] hover:bg-surface-2 hover:text-foreground hover:shadow-sm lg:inline-flex"
			aria-label="Search views, namespaces, and resources"
			onclick={onOpenSearch}
		>
			<Search class="size-3.5" aria-hidden="true" />
			<span>Search resources...</span>
			<kbd class="rounded border bg-muted px-1 py-px font-mono text-xs text-muted-foreground">
				{SEARCH_SHORTCUT_HINT}
			</kbd>
		</Button>
	</div>
</header>
