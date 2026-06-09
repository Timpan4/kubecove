import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

export function SettingsSection({
	title,
	children,
}: {
	title: string;
	children: ReactNode;
}) {
	return (
		<section className="flex flex-col gap-2">
			<h2 className="px-1 text-sm font-semibold text-foreground">{title}</h2>
			<div className="divide-y divide-border overflow-hidden rounded-xl border bg-card">
				{children}
			</div>
		</section>
	);
}

export function SettingsRow({
	title,
	description,
	children,
}: {
	title: string;
	description: string;
	children: ReactNode;
}) {
	return (
		<div className="flex min-h-14 items-center justify-between gap-6 px-4 py-3">
			<div className="min-w-0">
				<div className="text-sm font-medium text-foreground">{title}</div>
				<div className="mt-0.5 max-w-xl text-xs leading-relaxed text-muted-foreground">
					{description}
				</div>
			</div>
			<div className="shrink-0">{children}</div>
		</div>
	);
}

export function SegmentedControl<T extends string>({
	value,
	options,
	onChange,
	ariaLabel,
}: {
	value: T;
	options: ReadonlyArray<{ value: T; label: string }>;
	onChange: (value: T) => void;
	ariaLabel: string;
}) {
	return (
		<div
			role="radiogroup"
			aria-label={ariaLabel}
			className="flex rounded-lg bg-muted p-0.5"
		>
			{options.map((option) => (
				<button
					key={option.value}
					type="button"
					role="radio"
					aria-checked={option.value === value}
					className={cn(
						"h-7 rounded-md px-3 text-xs font-medium transition-colors focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-ring/50",
						option.value === value
							? "bg-background text-foreground shadow-sm"
							: "text-muted-foreground hover:text-foreground",
					)}
					onClick={() => onChange(option.value)}
				>
					{option.label}
				</button>
			))}
		</div>
	);
}
