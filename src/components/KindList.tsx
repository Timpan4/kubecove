import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Field, FieldLabel } from "@/components/ui/field";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { CLUSTER_SCOPED_KINDS, SUPPORTED_KINDS, type AnyKind } from "@/lib/types";
import { cn } from "@/lib/utils";

interface KindListProps {
	selectedKinds: AnyKind[];
	onToggleKind: (kind: AnyKind) => void;
}

function KindOption({
	kind,
	checked,
	onToggleKind,
}: {
	kind: AnyKind;
	checked: boolean;
	onToggleKind: (kind: AnyKind) => void;
}) {
	const checkboxId = `kind-${kind}`;
	return (
		<li
			className={cn(
				"cursor-pointer rounded-md p-2 text-sm transition-colors hover:bg-accent",
				checked && "bg-accent",
			)}
		>
			<Field orientation="horizontal" className="items-center gap-2">
				<Checkbox
					id={checkboxId}
					checked={checked}
					onCheckedChange={() => onToggleKind(kind)}
				/>
				<FieldLabel
					htmlFor={checkboxId}
					className="min-w-0 flex-1 cursor-pointer font-normal"
				>
					{kind}
				</FieldLabel>
			</Field>
		</li>
	);
}

export function KindList({ selectedKinds, onToggleKind }: KindListProps) {
	const allKinds = [...SUPPORTED_KINDS, ...CLUSTER_SCOPED_KINDS];
	const allSelected = selectedKinds.length === allKinds.length;

	const handleToggleAll = () => {
		if (allSelected) {
			allKinds.forEach((kind) => {
				if (selectedKinds.includes(kind)) {
					onToggleKind(kind);
				}
			});
		} else {
			allKinds.forEach((kind) => {
				if (!selectedKinds.includes(kind)) {
					onToggleKind(kind);
				}
			});
		}
	};

	return (
		<div className="flex min-h-0 flex-col">
			<div className="mb-3 flex items-center justify-between">
				<h3 className="m-0 text-xs font-semibold uppercase text-muted-foreground">
					Resource Kinds
				</h3>
				<Button
					onClick={handleToggleAll}
					type="button"
					variant="outline"
					size="sm"
					className="h-7 px-2 text-[0.625rem]"
				>
					{allSelected ? "Deselect All" : "Select All"}
				</Button>
			</div>
			<ScrollArea className="min-h-0 pr-2">
				<ul className="m-0 list-none p-0">
					{SUPPORTED_KINDS.map((kind) => (
						<KindOption
							key={kind}
							kind={kind}
							checked={selectedKinds.includes(kind)}
							onToggleKind={onToggleKind}
						/>
					))}
					{CLUSTER_SCOPED_KINDS.length > 0 && (
						<>
							<li className="py-2">
								<Separator />
							</li>
							{CLUSTER_SCOPED_KINDS.map((kind) => (
								<KindOption
									key={kind}
									kind={kind}
									checked={selectedKinds.includes(kind)}
									onToggleKind={onToggleKind}
								/>
							))}
						</>
					)}
				</ul>
			</ScrollArea>
		</div>
	);
}
