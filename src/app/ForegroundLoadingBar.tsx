import { useForegroundLoading } from "@/lib/foreground-loading";
import { cn } from "@/lib/utils";

export function ForegroundLoadingBar() {
	const activeLoads = useForegroundLoading();
	const active = activeLoads > 0;

	return (
		<div
			aria-hidden="true"
			className={cn(
				"h-0.5 overflow-hidden bg-transparent opacity-0 transition-opacity",
				active && "opacity-100",
			)}
		>
			<div className="foreground-loading-bar h-full w-1/3 rounded-full bg-primary" />
		</div>
	);
}
