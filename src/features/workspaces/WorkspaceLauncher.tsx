import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { FolderOpen, Pencil, Plus, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectGroup,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { createTauriClient, listKubeContexts, listNamespaces } from "@/lib/tauri";
import {
	DEFAULT_WORKSPACE_KINDS,
	makeWorkspaceShortcuts,
	summarizeWorkspaceScope,
	useWorkspaceStore,
	type SavedWorkspace,
} from "@/lib/workspaces";
import { cn } from "@/lib/utils";

interface WorkspaceLauncherProps {
	onOpenWorkspace: (workspace: SavedWorkspace) => void;
}

export function WorkspaceLauncher({ onOpenWorkspace }: WorkspaceLauncherProps) {
	const { workspaces, createWorkspace, updateWorkspace, deleteWorkspace } =
		useWorkspaceStore();
	const [editingId, setEditingId] = useState<string | null>(null);
	const [name, setName] = useState("");
	const [selectedContext, setSelectedContext] = useState("");
	const [selectedNamespaces, setSelectedNamespaces] = useState<string[]>([]);

	const contextsQuery = useQuery({
		queryKey: ["workspace-contexts"],
		queryFn: () => listKubeContexts(createTauriClient()),
	});
	const contexts = contextsQuery.data ?? [];
	const effectiveContext =
		selectedContext ||
		contexts.find((context) => context.isCurrent)?.name ||
		contexts[0]?.name ||
		"";

	const namespacesQuery = useQuery({
		queryKey: ["workspace-namespaces", effectiveContext],
		queryFn: () => listNamespaces(createTauriClient(), effectiveContext),
		enabled: effectiveContext.length > 0,
	});
	const namespaces = namespacesQuery.data ?? [];

	const selectedContextMissing =
		effectiveContext.length > 0 &&
		!contexts.some((context) => context.name === effectiveContext);
	const canCreate = effectiveContext.length > 0 && !selectedContextMissing;

	const sortedWorkspaces = useMemo(
		() =>
			[...workspaces].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)),
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

	const resetForm = () => {
		setEditingId(null);
		setName("");
		setSelectedContext("");
		setSelectedNamespaces([]);
	};

	const handleEdit = (workspace: SavedWorkspace) => {
		setEditingId(workspace.id);
		setName(workspace.name);
		setSelectedContext(workspace.scope.clusterContext);
		setSelectedNamespaces(workspace.scope.namespaces);
	};

	const handleSubmit = () => {
		if (!canCreate) return;
		const trimmedName = name.trim();
		if (editingWorkspace) {
			updateWorkspace(editingWorkspace.id, {
				name: trimmedName || effectiveContext,
				scope: {
					...editingWorkspace.scope,
					clusterContext: effectiveContext,
					namespaces: selectedNamespaces,
				},
				shortcuts: makeWorkspaceShortcuts(selectedNamespaces),
			});
			resetForm();
			return;
		}
		const workspace = createWorkspace({
			name: trimmedName || effectiveContext,
			clusterContext: effectiveContext,
			namespaces: selectedNamespaces,
			kinds: DEFAULT_WORKSPACE_KINDS,
		});
		resetForm();
		onOpenWorkspace(workspace);
	};

	return (
		<div className="flex h-screen w-full flex-col overflow-hidden bg-background text-foreground">
			<header className="flex h-12 shrink-0 items-center border-b bg-sidebar px-4">
				<div className="text-sm font-semibold">k8s-manager</div>
			</header>
			<main className="min-h-0 flex-1 overflow-y-auto p-4 md:p-6">
				<div className="mx-auto grid w-full max-w-6xl gap-4 lg:grid-cols-[minmax(0,1fr)_360px]">
					<section className="min-w-0">
						<div className="mb-3 flex items-center justify-between gap-3">
							<h1 className="text-base font-semibold">Workspaces</h1>
							<span className="text-xs text-muted-foreground">
								{sortedWorkspaces.length}
							</span>
						</div>
						<div className="grid gap-2">
							{contextsQuery.isPending && (
								<div className="rounded-md border bg-card p-4 text-xs text-muted-foreground">
									Loading contexts...
								</div>
							)}
							{contextsQuery.isError && (
								<div className="rounded-md border border-destructive/40 bg-card p-4 text-xs text-destructive">
									Failed to load contexts.
								</div>
							)}
							{sortedWorkspaces.length === 0 && !contextsQuery.isPending && (
								<div className="rounded-md border bg-card p-4 text-sm text-muted-foreground">
									No saved workspaces.
								</div>
							)}
							{sortedWorkspaces.map((workspace) => {
								const unavailable = !contexts.some(
									(context) => context.name === workspace.scope.clusterContext,
								);
								return (
									<article
										key={workspace.id}
										className={cn(
											"grid gap-3 rounded-md border bg-card p-4 md:grid-cols-[minmax(0,1fr)_auto]",
											unavailable && "border-amber-500/40",
										)}
									>
										<div className="min-w-0">
											<div className="flex min-w-0 items-center gap-2">
												<h2 className="truncate text-sm font-semibold">
													{workspace.name}
												</h2>
												{unavailable && (
													<span className="rounded-sm border border-amber-500/40 px-1.5 py-0.5 text-[0.6875rem] text-amber-300">
														Context unavailable
													</span>
												)}
											</div>
											<p className="mt-1 truncate text-xs text-muted-foreground">
												{summarizeWorkspaceScope(workspace.scope)}
											</p>
										</div>
										<div className="flex items-center justify-end gap-2">
											<Button
												type="button"
												variant="outline"
												size="sm"
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
										</div>
									</article>
								);
							})}
						</div>
					</section>

					<section className="rounded-md border bg-card p-4">
						<div className="mb-4 flex items-center justify-between gap-2">
							<h2 className="text-sm font-semibold">
								{editingWorkspace ? "Edit workspace" : "New workspace"}
							</h2>
							{editingWorkspace && (
								<Button
									type="button"
									variant="ghost"
									size="icon-sm"
									aria-label="Cancel edit"
									onClick={resetForm}
								>
									<X />
								</Button>
							)}
						</div>
						<div className="grid gap-3">
							<label className="grid gap-1.5">
								<span className="text-xs font-medium text-muted-foreground">
									Name
								</span>
								<Input
									value={name}
									placeholder={effectiveContext || "Workspace name"}
									onChange={(event) => setName(event.target.value)}
								/>
							</label>
							<label className="grid gap-1.5">
								<span className="text-xs font-medium text-muted-foreground">
									Context
								</span>
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
							</label>
							<div className="grid gap-1.5">
								<div className="text-xs font-medium text-muted-foreground">
									Namespaces
								</div>
								<div className="max-h-52 overflow-y-auto rounded-md border bg-background/40 p-1">
									{namespacesQuery.isPending && effectiveContext && (
										<div className="px-2 py-1.5 text-xs text-muted-foreground">
											Loading namespaces...
										</div>
									)}
									{!namespacesQuery.isPending && namespaces.length === 0 && (
										<div className="px-2 py-1.5 text-xs text-muted-foreground">
											All namespaces
										</div>
									)}
									{namespaces.map((namespace) => (
										<label
											key={namespace.name}
											className="flex h-7 items-center gap-2 rounded-sm px-2 text-xs hover:bg-muted"
										>
											<input
												type="checkbox"
												checked={selectedNamespaces.includes(namespace.name)}
												onChange={() => toggleNamespace(namespace.name)}
											/>
											<span className="min-w-0 truncate">{namespace.name}</span>
										</label>
									))}
								</div>
							</div>
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
						</div>
					</section>
				</div>
			</main>
		</div>
	);
}
