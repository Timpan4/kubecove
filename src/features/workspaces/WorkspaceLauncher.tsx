import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { FolderOpen, Pencil, Plus, Trash2, X } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
	Card,
	CardAction,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
import {
	Field,
	FieldGroup,
	FieldLabel,
	FieldLegend,
	FieldSet,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
	Select,
	SelectContent,
	SelectGroup,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { createTauriClient, listKubeContexts, listNamespaces } from "@/lib/tauri";
import { queryKeys } from "@/lib/queryKeys";
import { useSettingsState } from "@/lib/settings";
import {
	DEFAULT_WORKSPACE_KINDS,
	createWorkspaceScope,
	makeWorkspaceShortcuts,
	summarizeWorkspaceScope,
	useWorkspaceStore,
	workspaceScopeContexts,
	type SavedWorkspace,
} from "@/lib/workspaces";
import { cnfast } from "@/lib/utils";
import { WorkspaceContextGroupField } from "./WorkspaceContextGroupField";

interface WorkspaceLauncherProps {
	onOpenWorkspace: (workspace: SavedWorkspace) => void;
}

export function WorkspaceLauncher({ onOpenWorkspace }: WorkspaceLauncherProps) {
	const { workspaces, createWorkspace, updateWorkspace, deleteWorkspace } =
		useWorkspaceStore();
	const [editingId, setEditingId] = useState<string | null>(null);
	const [name, setName] = useState("");
	const [selectedContext, setSelectedContext] = useState("");
	const [selectedGroupContexts, setSelectedGroupContexts] = useState<string[]>([]);
	const [selectedNamespaces, setSelectedNamespaces] = useState<string[]>([]);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);

	const {
		data: contextsData,
		isPending: contextsPending,
		isError: contextsError,
	} = useQuery({
		queryKey: queryKeys.kubeContexts(kubeconfigEnvVar),
		queryFn: () => listKubeContexts(createTauriClient(), kubeconfigEnvVar),
	});
	const contexts = contextsData ?? [];
	const effectiveContext =
		selectedContext ||
		contexts.find((context) => context.isCurrent)?.name ||
		contexts[0]?.name ||
		"";

	const {
		data: namespacesData,
		isPending: namespacesPending,
		isError: namespacesError,
		refetch: refetchNamespaces,
	} = useQuery({
		queryKey: queryKeys.namespaces(effectiveContext, kubeconfigEnvVar),
		queryFn: () =>
			listNamespaces(createTauriClient(), effectiveContext, kubeconfigEnvVar),
		enabled: effectiveContext.length > 0,
	});
	const namespaces = namespacesData ?? [];
	const selectedClusterContexts = useMemo(
		() =>
			Array.from(
				new Set([effectiveContext, ...selectedGroupContexts].filter(Boolean)),
			),
		[effectiveContext, selectedGroupContexts],
	);

	const selectedContextMissing =
		effectiveContext.length > 0 &&
		!contexts.some((context) => context.name === effectiveContext);
	const canCreate = effectiveContext.length > 0 && !selectedContextMissing;

	const sortedWorkspaces = useMemo(
		() => workspaces.toSorted((a, b) => b.updatedAt.localeCompare(a.updatedAt)),
		[workspaces],
	);
	const editingWorkspace =
		workspaces.find((workspace) => workspace.id === editingId) ?? null;

	const toggleNamespace = (namespace: string) => {
		setSelectedNamespaces((current) =>
			current.includes(namespace)
				? current.filter((item) => item !== namespace)
				: [...current, namespace],
		);
	};
	const toggleGroupContext = (context: string) => {
		if (context === effectiveContext) return;
		setSelectedGroupContexts((current) =>
			current.includes(context)
				? current.filter((item) => item !== context)
				: [...current, context],
		);
	};

	const resetForm = () => {
		setEditingId(null);
		setName("");
		setSelectedContext("");
		setSelectedGroupContexts([]);
		setSelectedNamespaces([]);
	};

	const handleEdit = (workspace: SavedWorkspace) => {
		setEditingId(workspace.id);
		setName(workspace.name);
		setSelectedContext(workspace.scope.clusterContext);
		setSelectedGroupContexts(
			workspaceScopeContexts(workspace.scope).filter(
				(context) => context !== workspace.scope.clusterContext,
			),
		);
		setSelectedNamespaces(workspace.scope.namespaces);
	};

	const handleSubmit = () => {
		if (!canCreate) return;
		const trimmedName = name.trim();
		const clusterGroupName =
			selectedClusterContexts.length > 1
				? `${trimmedName || effectiveContext} group`
				: undefined;
		if (editingWorkspace) {
			const scope = createWorkspaceScope({
				name: trimmedName || effectiveContext,
				clusterContext: effectiveContext,
				clusterContexts: selectedClusterContexts,
				clusterGroupName,
				namespaces: selectedNamespaces,
				kinds: editingWorkspace.scope.kinds,
				shortcutPreferences: editingWorkspace.scope.shortcutPreferences,
			});
			updateWorkspace(editingWorkspace.id, {
				name: trimmedName || effectiveContext,
				scope: {
					...scope,
					gitOpsFilter:
						editingWorkspace.scope.gitOpsFilter ??
						editingWorkspace.scope.argoAppFilter,
					argoAppFilter: editingWorkspace.scope.argoAppFilter,
					layout: editingWorkspace.scope.layout,
				},
				shortcuts: makeWorkspaceShortcuts(
					scope.namespaces,
					undefined,
					scope.shortcutPreferences,
					scope,
				),
			});
			resetForm();
			return;
		}
		const workspace = createWorkspace({
			name: trimmedName || effectiveContext,
			clusterContext: effectiveContext,
			clusterContexts: selectedClusterContexts,
			clusterGroupName,
			namespaces: selectedNamespaces,
			kinds: DEFAULT_WORKSPACE_KINDS,
		});
		resetForm();
		onOpenWorkspace(workspace);
	};

	return (
		<div className="flex h-full w-full flex-col overflow-hidden bg-background text-foreground">
			<main className="min-h-0 flex-1 overflow-y-auto p-4 md:p-6">
				<div className="flex min-h-full flex-col justify-center">
				<div className="mx-auto grid w-full max-w-6xl gap-4 lg:grid-cols-[minmax(0,1fr)_360px]">
					<section className="min-w-0">
						<div className="mb-3 flex items-baseline gap-2">
							<h1 className="text-base font-semibold">Workspaces</h1>
							<span className="text-xs text-muted-foreground">
								{sortedWorkspaces.length === 1
									? "1 saved"
									: `${sortedWorkspaces.length} saved`}
							</span>
						</div>
						<div className="grid gap-2">
							{contextsPending && (
								<Card size="sm">
									<CardContent className="text-xs text-muted-foreground">
											Loading contexts…
									</CardContent>
								</Card>
							)}
							{contextsError && (
								<Alert variant="destructive">
									<AlertTitle>Failed to load contexts</AlertTitle>
									<AlertDescription>
										Refresh the workspace list after your kubeconfig is available.
									</AlertDescription>
								</Alert>
							)}
							{sortedWorkspaces.length === 0 && !contextsPending && (
								<Empty className="min-h-40 border">
									<EmptyHeader>
										<EmptyTitle>No saved workspaces</EmptyTitle>
										<EmptyDescription>
											Create a workspace from a cluster context to start.
										</EmptyDescription>
									</EmptyHeader>
								</Empty>
							)}
							{sortedWorkspaces.map((workspace) => {
								const unavailable = !contexts.some(
									(context) => context.name === workspace.scope.clusterContext,
								);
								return (
									<Card
										key={workspace.id}
										size="sm"
										className={cnfast(
											"grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]",
											unavailable && "border-amber-500/40",
										)}
									>
										<CardHeader className="min-w-0">
											<div className="flex min-w-0 items-center gap-2">
												<CardTitle className="truncate">
													{workspace.name}
												</CardTitle>
												{unavailable && (
													<span className="rounded-sm border border-amber-500/40 px-1.5 py-0.5 text-[0.6875rem] text-amber-300">
														Context unavailable
													</span>
												)}
											</div>
											<CardDescription className="truncate">
												{summarizeWorkspaceScope(workspace.scope)}
											</CardDescription>
										</CardHeader>
										<CardAction
											className="row-start-auto flex items-center justify-end gap-2 px-3"
											role="group"
											aria-label={`${workspace.name} actions`}
										>
											<Button
												type="button"
												size="sm"
												aria-label={`Open ${workspace.name}`}
												onClick={() => onOpenWorkspace(workspace)}
											>
												<FolderOpen data-icon="inline-start" />
												Open
											</Button>
											<Button
												type="button"
												variant="ghost"
												size="icon-sm"
												aria-label={`Edit ${workspace.name}`}
												onClick={() => handleEdit(workspace)}
											>
												<Pencil />
											</Button>
											<Button
												type="button"
												variant="ghost"
												size="icon-sm"
												aria-label={`Delete ${workspace.name}`}
												onClick={() => {
													deleteWorkspace(workspace.id);
													if (editingId === workspace.id) resetForm();
												}}
											>
												<Trash2 />
											</Button>
										</CardAction>
									</Card>
								);
							})}
						</div>
					</section>

					<Card size="sm">
						<CardHeader>
							<CardTitle>
								{editingWorkspace ? "Edit workspace" : "New workspace"}
							</CardTitle>
							{editingWorkspace && (
								<CardAction>
									<Button
										type="button"
										variant="ghost"
										size="icon-sm"
										aria-label="Cancel edit"
										onClick={resetForm}
									>
										<X />
									</Button>
								</CardAction>
							)}
						</CardHeader>
						<CardContent>
							<FieldGroup className="gap-3">
								<Field>
									<FieldLabel htmlFor="workspace-name">Name</FieldLabel>
								<Input
									id="workspace-name"
									value={name}
									placeholder={effectiveContext || "Workspace name"}
									onChange={(event) => setName(event.target.value)}
								/>
								</Field>
								<Field>
									<FieldLabel>Context</FieldLabel>
								<Select
									value={effectiveContext}
									onValueChange={(value) => {
										setSelectedContext(value);
										setSelectedNamespaces([]);
									}}
								>
									<SelectTrigger className="w-full">
										<SelectValue placeholder="Select context" />
									</SelectTrigger>
									<SelectContent>
										<SelectGroup>
											{contexts.map((context) => (
												<SelectItem key={context.name} value={context.name}>
													{context.name}
												</SelectItem>
											))}
										</SelectGroup>
									</SelectContent>
								</Select>
								</Field>
								<WorkspaceContextGroupField
									items={contexts}
									primaryContext={effectiveContext}
									selectedNames={selectedClusterContexts}
									onToggleContext={toggleGroupContext}
								/>
								<FieldSet className="gap-1.5">
									<FieldLegend
										variant="label"
										className="text-muted-foreground"
									>
										Namespaces
									</FieldLegend>
								<ScrollArea className="h-52 rounded-md border bg-background/40">
									<div className="p-1">
									{namespacesPending && effectiveContext && (
										<div className="px-2 py-1.5 text-xs text-muted-foreground">
												Loading namespaces…
										</div>
									)}
									{namespacesError && (
										<div className="grid gap-2 px-2 py-1.5 text-xs text-destructive">
											<span>
												Failed to load namespaces. You can still save an all-namespace workspace.
											</span>
											<Button
												type="button"
												variant="outline"
												size="sm"
												className="w-fit"
												onClick={() => refetchNamespaces()}
											>
												Retry
											</Button>
										</div>
									)}
									{!namespacesPending &&
										!namespacesError &&
										namespaces.length === 0 && (
										<div className="px-2 py-1.5 text-xs text-muted-foreground">
											All namespaces
										</div>
									)}
									{!namespacesError && namespaces.map((namespace) => {
										const checkboxId = `workspace-namespace-${namespace.name}`;
										return (
											<Field
												key={namespace.name}
												orientation="horizontal"
												className="h-7 items-center gap-2 rounded-sm px-2 text-xs hover:bg-muted"
											>
												<Checkbox
													id={checkboxId}
													checked={selectedNamespaces.includes(namespace.name)}
													onCheckedChange={() => toggleNamespace(namespace.name)}
												/>
												<FieldLabel
													htmlFor={checkboxId}
													className="min-w-0 flex-1 cursor-pointer truncate font-normal"
												>
													{namespace.name}
												</FieldLabel>
											</Field>
										);
									})}
									</div>
								</ScrollArea>
								</FieldSet>
							<Button
								type="button"
								size="lg"
								disabled={!canCreate}
								onClick={handleSubmit}
							>
								{editingWorkspace ? (
									<FolderOpen data-icon="inline-start" />
								) : (
									<Plus data-icon="inline-start" />
								)}
								{editingWorkspace ? "Save workspace" : "Create workspace"}
							</Button>
							</FieldGroup>
						</CardContent>
					</Card>
				</div>
				</div>
			</main>
		</div>
	);
}
