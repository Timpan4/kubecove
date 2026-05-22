import { Checkbox } from "@/components/ui/checkbox";
import { Field, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { ClusterContext } from "@/lib/types";

interface WorkspaceContextGroupFieldProps {
	items: ClusterContext[];
	primaryContext: string;
	selectedNames: string[];
	onToggleContext: (context: string) => void;
}

export function WorkspaceContextGroupField({
	items,
	primaryContext,
	selectedNames,
	onToggleContext,
}: WorkspaceContextGroupFieldProps) {
	if (items.length <= 1) return null;
	return (
		<FieldSet className="gap-1.5">
			<FieldLegend variant="label" className="text-muted-foreground">
				Cluster group
			</FieldLegend>
			<ScrollArea className="h-32 rounded-md border bg-background/40">
				<div className="p-1">
					{items.map((context) => {
						const checkboxId = `workspace-context-${context.name}`;
						const checked =
							context.name === primaryContext ||
							selectedNames.includes(context.name);
						return (
							<Field
								key={context.name}
								orientation="horizontal"
								className="h-7 items-center gap-2 rounded-sm px-2 text-xs hover:bg-muted"
							>
								<Checkbox
									id={checkboxId}
									checked={checked}
									disabled={context.name === primaryContext}
									onCheckedChange={() => onToggleContext(context.name)}
								/>
								<FieldLabel
									htmlFor={checkboxId}
									className="min-w-0 flex-1 cursor-pointer truncate font-normal"
								>
									{context.name}
								</FieldLabel>
							</Field>
						);
					})}
				</div>
			</ScrollArea>
		</FieldSet>
	);
}
