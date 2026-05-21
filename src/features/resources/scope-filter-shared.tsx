import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Field, FieldLabel } from "@/components/ui/field";
import {
	Popover,
	PopoverContent,
	PopoverTrigger,
} from "@/components/ui/popover";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "@/lib/utils";
import type { ScopePill } from "./helpers";

export function EditableScopePill({
	pill,
	children,
}: {
	pill: ScopePill;
	children: ReactNode;
}) {
	return (
		<Popover>
			<PopoverTrigger asChild>
				<button
					type="button"
					className="flex h-8 max-w-full items-center gap-1.5 rounded-sm border border-slate-700/80 bg-slate-950/45 px-2.5 text-xs shadow-none transition-colors hover:border-ring hover:bg-muted/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 data-[state=open]:border-primary/60"
					aria-label={`Edit ${pill.label.toLowerCase()} filter`}
				>
					<span className="text-muted-foreground">{pill.label}</span>
					<strong className="min-w-0 truncate font-semibold text-foreground">
						{pill.value}
					</strong>
				</button>
			</PopoverTrigger>
			<PopoverContent align="start" className="w-80 p-3">
				{children}
			</PopoverContent>
		</Popover>
	);
}

export function PickerHeader({
	title,
	allSelected,
	onToggleAll,
}: {
	title: string;
	allSelected: boolean;
	onToggleAll: () => void;
}) {
	return (
		<div className="mb-3 flex items-center justify-between">
			<h3 className="m-0 text-xs font-semibold uppercase text-muted-foreground">
				{title}
			</h3>
			<Button
				type="button"
				variant="outline"
				size="sm"
				className="h-7 px-2 text-[0.625rem]"
				onClick={onToggleAll}
			>
				{allSelected ? "Deselect All" : "Select All"}
			</Button>
		</div>
	);
}

export function PickerStatus({
	loading,
	loadingLabel,
	error,
	onRetry,
}: {
	loading: boolean;
	loadingLabel: string;
	error: string | null;
	onRetry: () => void;
}) {
	if (loading) {
		return (
			<div className="mb-3 flex items-center gap-2 text-sm text-muted-foreground">
				<Spinner className="size-4" />
				{loadingLabel}
			</div>
		);
	}
	if (!error) return null;
	return (
		<div className="mb-3 flex flex-col gap-2 text-sm text-destructive">
			<span>{error}</span>
			<Button
				type="button"
				variant="outline"
				size="sm"
				className="w-fit"
				onClick={onRetry}
			>
				Retry
			</Button>
		</div>
	);
}

export function ScopeOption({
	id,
	label,
	checked,
	onToggle,
}: {
	id: string;
	label: string;
	checked: boolean;
	onToggle: () => void;
}) {
	return (
		<li
			className={cn(
				"rounded-md p-2 text-sm transition-colors hover:bg-accent",
				checked && "bg-accent",
			)}
		>
			<Field orientation="horizontal" className="items-center gap-2">
				<Checkbox id={id} checked={checked} onCheckedChange={onToggle} />
				<FieldLabel
					htmlFor={id}
					className="min-w-0 flex-1 cursor-pointer truncate font-normal"
				>
					{label}
				</FieldLabel>
			</Field>
		</li>
	);
}
