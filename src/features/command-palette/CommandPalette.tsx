import { useMemo, useState } from "react";
import { Box, Folder, PanelsTopLeft } from "lucide-react";
import {
	CommandDialog,
	CommandEmpty,
	CommandGroup,
	CommandInput,
	CommandItem,
	CommandList,
} from "@/components/ui/command";
import { filterResourceSearchIndex } from "@/features/resources";
import { useDashboardState } from "@/lib/hooks";
import { makeNamespaceNode } from "@/lib/tree-nav";
import type { ResourceSummary } from "@/lib/types";
import type { SavedWorkspace } from "@/lib/workspaces";
import type { AppNavigation } from "@/app/useAppNavigation";
import {
	buildNavigationEntries,
	filterNamespaces,
	filterNavigationEntries,
	resourceEntryKey,
	type PaletteNavigationEntry,
} from "./entries";
import { useCommandPaletteStore } from "./store";
import { useCommandPaletteShortcut } from "./useCommandPaletteShortcut";
import { useResourceSearchEntries } from "./useResourceSearchEntries";

const RESOURCE_RESULT_CAP = 50;
const NAMESPACE_RESULT_CAP = 8;

interface CommandPaletteProps {
	clusterContext: string;
	kubeconfigEnvVar?: string;
	workspace: SavedWorkspace | null;
	navigation: AppNavigation;
	onSelectResource: (resource: ResourceSummary) => void;
}

export function CommandPalette({
	clusterContext,
	kubeconfigEnvVar,
	workspace,
	navigation,
	onSelectResource,
}: CommandPaletteProps) {
	useCommandPaletteShortcut();
	const open = useCommandPaletteStore((state) => state.open);
	const setOpen = useCommandPaletteStore((state) => state.setOpen);
	const argoDetected = useDashboardState().argoDetected;
	const [query, setQuery] = useState("");

	const { index, namespaces } = useResourceSearchEntries({
		open,
		clusterContext,
		kubeconfigEnvVar,
		workspace,
	});

	const navigationEntries = useMemo(
		() => buildNavigationEntries(argoDetected),
		[argoDetected],
	);
	const visibleNavigation = useMemo(
		() => filterNavigationEntries(navigationEntries, query),
		[navigationEntries, query],
	);
	const hasQuery = query.trim().length > 0;
	const visibleNamespaces = useMemo(
		() =>
			hasQuery
				? filterNamespaces(namespaces, query).slice(0, NAMESPACE_RESULT_CAP)
				: [],
		[hasQuery, namespaces, query],
	);
	const visibleResources = useMemo(
		() =>
			hasQuery
				? filterResourceSearchIndex(index, query, "").slice(
						0,
						RESOURCE_RESULT_CAP,
					)
				: [],
		[hasQuery, index, query],
	);

	const close = () => {
		setOpen(false);
		setQuery("");
	};

	const handleOpenChange = (next: boolean) => {
		setOpen(next);
		if (!next) setQuery("");
	};

	const selectNavigation = (entry: PaletteNavigationEntry) => {
		close();
		if (entry.action === "settings") {
			navigation.handleOpenSettings();
		} else if (entry.action === "launcher") {
			navigation.handleOpenLauncher();
		} else if (entry.nodeId) {
			navigation.handleTreeNodeSelect(entry.nodeId);
		}
	};

	const selectNamespace = (namespace: string) => {
		close();
		navigation.handleTreeNodeSelect(makeNamespaceNode(namespace).id);
	};

	const selectResource = (resource: ResourceSummary) => {
		close();
		onSelectResource(resource);
	};

	return (
		<CommandDialog
			open={open}
			onOpenChange={handleOpenChange}
			title="Search"
			description="Search views, namespaces, and resources"
			commandProps={{ shouldFilter: false }}
		>
			<CommandInput
				placeholder="Search views, namespaces, and resources..."
				value={query}
				onValueChange={setQuery}
			/>
			<CommandList>
				<CommandEmpty>No results found.</CommandEmpty>
				{visibleResources.length > 0 && (
					<CommandGroup heading="Resources">
						{visibleResources.map((resource) => (
							<CommandItem
								key={resourceEntryKey(resource)}
								value={resourceEntryKey(resource)}
								onSelect={() => selectResource(resource)}
							>
								<Box className="shrink-0 text-muted-foreground" />
								<span className="truncate">{resource.name}</span>
								<span className="ml-auto flex shrink-0 items-center gap-2 text-[10px] text-muted-foreground">
									{resource.namespace && <span>{resource.namespace}</span>}
									<span>{resource.kind}</span>
								</span>
							</CommandItem>
						))}
					</CommandGroup>
				)}
				{visibleNamespaces.length > 0 && (
					<CommandGroup heading="Namespaces">
						{visibleNamespaces.map((namespace) => (
							<CommandItem
								key={`namespace:${namespace}`}
								value={`namespace:${namespace}`}
								onSelect={() => selectNamespace(namespace)}
							>
								<Folder className="shrink-0 text-muted-foreground" />
								<span className="truncate">{namespace}</span>
							</CommandItem>
						))}
					</CommandGroup>
				)}
				{visibleNavigation.length > 0 && (
					<CommandGroup heading="Go to">
						{visibleNavigation.map((entry) => (
							<CommandItem
								key={entry.id}
								value={entry.id}
								onSelect={() => selectNavigation(entry)}
							>
								<PanelsTopLeft className="shrink-0 text-muted-foreground" />
								<span className="truncate">{entry.label}</span>
							</CommandItem>
						))}
					</CommandGroup>
				)}
			</CommandList>
		</CommandDialog>
	);
}
