import { Children, createContext, isValidElement, useContext } from "react";
import type { ReactNode } from "react";
import { cnfast } from "@/lib/utils";

export interface SettingsRowMeta {
	title: string;
	description: string;
}

export const SettingsSearchContext = createContext("");

export function matchesSettingsQuery(query: string, meta: SettingsRowMeta): boolean {
	const text = query.trim().toLowerCase();
	if (!text) return true;
	return `${meta.title} ${meta.description}`.toLowerCase().includes(text);
}

function rowMeta(child: unknown): SettingsRowMeta | null {
	if (!isValidElement(child)) return null;
	const props = child.props as { title?: unknown; description?: unknown };
	if (typeof props.title !== "string") return null;
	return {
		title: props.title,
		description: typeof props.description === "string" ? props.description : "",
	};
}

export function SettingsSection({
	title,
	showTitle = true,
	children,
}: {
	title: string;
	showTitle?: boolean;
	children: ReactNode;
}) {
	const query = useContext(SettingsSearchContext).trim();
	const items = Children.toArray(children);
	const visible = query
		? items.filter((child) => {
				const meta = rowMeta(child);
				return meta !== null && matchesSettingsQuery(query, meta);
			})
		: items;
	if (visible.length === 0) return null;
	return (
		<section className="flex flex-col gap-2">
			{showTitle && (
				<h2 className="px-1 text-sm font-semibold text-foreground">{title}</h2>
			)}
			<div className="divide-y divide-border overflow-hidden rounded-xl border bg-card">
				{visible}
			</div>
		</section>
	);
}

export function SettingsRow({
	title,
	description,
	children,
}: SettingsRowMeta & { children: ReactNode }) {
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

export function SettingsBlock({
	title,
	description,
	actions,
	children,
}: SettingsRowMeta & { actions?: ReactNode; children: ReactNode }) {
	return (
		<div className="flex flex-col gap-3 px-4 py-3">
			<div className="flex items-center justify-between gap-3">
				<div className="min-w-0">
					<div className="text-sm font-medium text-foreground">{title}</div>
					{description && (
						<div className="mt-0.5 max-w-xl text-xs leading-relaxed text-muted-foreground">
							{description}
						</div>
					)}
				</div>
				{actions && <div className="shrink-0">{actions}</div>}
			</div>
			{children}
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
					className={cnfast(
						"h-7 cursor-pointer rounded-md px-3 text-xs font-medium transition-colors focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-ring/50",
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
