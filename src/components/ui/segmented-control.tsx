import type * as React from "react"
import { cn } from "@/lib/utils"

/**
 * Segmented multi-choice control. Renders as a muted track with a raised active pill.
 * Used for compact runtime/theme/view switches where a Select is too heavy.
 * Finite transitions only — safe inside transformed containers (no infinite animation).
 */
export function SegmentedControl<T extends string>({
	value,
	options,
	onChange,
	ariaLabel,
	size = "sm",
	className,
}: {
	value: T
	options: ReadonlyArray<{
		value: T
		label: string
		icon?: React.ReactNode
	}>
	onChange: (value: T) => void
	ariaLabel: string
	size?: "sm" | "md"
	className?: string
}) {
	return (
		<div
			role="radiogroup"
			aria-label={ariaLabel}
			className={cn(
				"inline-flex items-center rounded-lg bg-surface-0 p-0.5 ring-1 ring-border/60",
				className
			)}
		>
			{options.map((option) => {
				const active = option.value === value
				return (
					<button
						key={option.value}
						type="button"
						role="radio"
						aria-checked={active}
						onClick={() => onChange(option.value)}
						className={cn(
							"inline-flex cursor-pointer items-center gap-1.5 rounded-md font-medium transition-all duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40 [&_svg]:pointer-events-none [&_svg]:size-3.5",
							size === "md" ? "h-7 px-2.5 text-xs" : "h-6 px-2 text-xs",
							active
								? "bg-surface-1 text-foreground shadow-sm"
								: "text-muted-foreground hover:text-foreground"
						)}
					>
						{option.icon}
						<span>{option.label}</span>
					</button>
				)
			})}
		</div>
	)
}
