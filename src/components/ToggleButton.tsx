import { cn } from "@/lib/utils";

interface ToggleButtonProps {
	checked: boolean;
	onCheckedChange: (checked: boolean) => void;
	ariaLabel: string;
}

export function ToggleButton({
	checked,
	onCheckedChange,
	ariaLabel,
}: ToggleButtonProps) {
	return (
		<button
			type="button"
			role="switch"
			aria-checked={checked}
			aria-label={ariaLabel}
			className={cn(
				"flex h-6 w-11 cursor-pointer items-center rounded-full border px-0.5 transition-colors focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-ring/50",
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
