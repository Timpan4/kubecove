import type { ReactNode } from "react";
import { Check } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
	type TimestampTimezone,
	useSettingsState,
} from "@/lib/settings";
import { cn } from "@/lib/utils";

function SettingsRow({
	title,
	description,
	children,
}: {
	title: string;
	description: string;
	children: ReactNode;
}) {
	return (
		<div className="flex min-h-16 items-center justify-between gap-6 border-b py-4">
			<div className="min-w-0">
				<div className="text-sm font-medium text-foreground">{title}</div>
				<div className="mt-1 max-w-2xl text-xs leading-relaxed text-muted-foreground">
					{description}
				</div>
			</div>
			<div className="shrink-0">{children}</div>
		</div>
	);
}

function ToggleButton({
	checked,
	onCheckedChange,
	ariaLabel,
}: {
	checked: boolean;
	onCheckedChange: (checked: boolean) => void;
	ariaLabel: string;
}) {
	return (
		<button
			type="button"
			role="switch"
			aria-checked={checked}
			aria-label={ariaLabel}
			className={cn(
				"flex h-6 w-11 items-center rounded-full border px-0.5 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
				checked
					? "border-primary bg-primary"
					: "border-border bg-muted",
			)}
			onClick={() => onCheckedChange(!checked)}
		>
			<span
				className={cn(
					"block size-5 rounded-full bg-background shadow-sm transition-transform",
					checked && "translate-x-5",
				)}
			/>
		</button>
	);
}

function TimezoneOption({
	value,
	label,
	selected,
	onSelect,
}: {
	value: TimestampTimezone;
	label: string;
	selected: boolean;
	onSelect: (value: TimestampTimezone) => void;
}) {
	return (
		<Button
			type="button"
			variant={selected ? "secondary" : "ghost"}
			size="sm"
			className="h-8 rounded-sm px-2.5"
			onClick={() => onSelect(value)}
			aria-pressed={selected}
		>
			{selected && <Check data-icon="inline-start" />}
			{label}
		</Button>
	);
}

export function SettingsPage() {
	const {
		showExactTimestamps,
		timestampTimezone,
		setShowExactTimestamps,
		setTimestampTimezone,
	} = useSettingsState();

	return (
		<div className="mx-auto flex w-full max-w-4xl flex-col gap-6">
			<header className="border-b pb-4">
				<h1 className="text-lg font-semibold text-foreground">Settings</h1>
			</header>

			<section className="flex flex-col">
				<SettingsRow
					title="Show exact timestamps"
					description="Adds the exact timestamp next to relative ages. Exact timestamps remain available in tooltips when this is off."
				>
					<ToggleButton
						checked={showExactTimestamps}
						onCheckedChange={setShowExactTimestamps}
						ariaLabel="Show exact timestamps"
					/>
				</SettingsRow>
				<SettingsRow
					title="Timestamp timezone"
					description="Controls exact timestamps in inline labels and tooltips."
				>
					<div className="flex rounded-md border bg-background/50 p-0.5">
						<TimezoneOption
							value="local"
							label="Local"
							selected={timestampTimezone === "local"}
							onSelect={setTimestampTimezone}
						/>
						<TimezoneOption
							value="utc"
							label="UTC"
							selected={timestampTimezone === "utc"}
							onSelect={setTimestampTimezone}
						/>
					</div>
				</SettingsRow>
			</section>
		</div>
	);
}
