import { useQuery } from "@tanstack/react-query";
import { TimestampText } from "@/components/TimestampText";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Field, FieldLabel } from "@/components/ui/field";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { createTauriClient, listNamespaces } from "@/lib/tauri";
import { queryKeys } from "@/lib/queryKeys";
import { useSettingsState } from "@/lib/settings";
import { cnfast } from "@/lib/utils";

interface NamespaceListProps {
	clusterContext: string;
	selectedNamespaces: string[];
	onNamespaceChange: (namespaces: string[]) => void;
}

export function NamespaceList({
	clusterContext,
	selectedNamespaces,
	onNamespaceChange,
}: NamespaceListProps) {
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	const {
		data: namespaces = [],
		isPending: loading,
		isError,
		error,
		refetch,
	} = useQuery({
		queryKey: queryKeys.namespaces(clusterContext, kubeconfigEnvVar),
		queryFn: () =>
			listNamespaces(createTauriClient(), clusterContext, kubeconfigEnvVar),
		enabled: Boolean(clusterContext),
	});

	const handleToggleAll = () => {
		if (selectedNamespaces.length === namespaces.length) {
			onNamespaceChange([]);
		} else {
			onNamespaceChange(namespaces.map((ns) => ns.name));
		}
	};

	const handleToggleOne = (name: string) => {
		if (selectedNamespaces.includes(name)) {
			onNamespaceChange(selectedNamespaces.filter((namespace) => namespace !== name));
		} else {
			onNamespaceChange([...selectedNamespaces, name]);
		}
	};

	if (!clusterContext) {
		return (
			<div className="text-sm text-muted-foreground">
				Select a cluster context first
			</div>
		);
	}

	if (loading) {
		return (
			<div className="flex items-center gap-2 text-sm text-muted-foreground">
				<Spinner className="size-4" />
					Loading namespaces…
			</div>
		);
	}

	if (isError) {
		const errorMessage =
			error instanceof Error ? error.message : "Failed to load namespaces";
		return (
			<Alert variant="destructive">
				<AlertTitle>Failed to load namespaces</AlertTitle>
				<AlertDescription className="flex flex-col gap-2">
					<span>{errorMessage}</span>
					<Button
						type="button"
						variant="outline"
						size="sm"
						className="w-fit"
						onClick={() => void refetch()}
					>
						Retry
					</Button>
				</AlertDescription>
			</Alert>
		);
	}

	if (namespaces.length === 0) {
		return <div className="text-sm text-muted-foreground">No namespaces found</div>;
	}

	const allSelected =
		namespaces.length > 0 && selectedNamespaces.length === namespaces.length;

	return (
		<div className="flex min-h-0 flex-col">
			<div className="mb-3 flex items-center justify-between">
				<h3 className="m-0 text-xs font-semibold uppercase text-muted-foreground">
					Namespaces
				</h3>
				<Button
					onClick={handleToggleAll}
					type="button"
					variant="outline"
					size="sm"
					className="h-7 px-2 text-xs"
				>
					{allSelected ? "Deselect All" : "Select All"}
				</Button>
			</div>
			<ScrollArea className="min-h-0 pr-2">
				<ul className="m-0 list-none p-0">
					{namespaces.map((namespace) => {
						const checked = selectedNamespaces.includes(namespace.name);
						const checkboxId = `namespace-${namespace.name}`;
						return (
							<li
								key={namespace.name}
								className={cnfast(
									"cursor-pointer rounded-md p-2 text-sm transition-colors hover:bg-accent",
									checked && "bg-accent",
								)}
							>
								<Field orientation="horizontal" className="items-center gap-2">
									<Checkbox
										id={checkboxId}
										checked={checked}
										onCheckedChange={() => handleToggleOne(namespace.name)}
									/>
									<FieldLabel
										htmlFor={checkboxId}
										className="min-w-0 flex-1 cursor-pointer font-normal"
									>
										<span className="flex-1 truncate">{namespace.name}</span>
										<TimestampText
											relative={namespace.age}
											exact={namespace.createdAt}
											className="text-xs text-muted-foreground outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
										/>
									</FieldLabel>
								</Field>
							</li>
						);
					})}
				</ul>
			</ScrollArea>
		</div>
	);
}
