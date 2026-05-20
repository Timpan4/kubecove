import { Spinner } from "@/components/ui/spinner";

export function ViewLoadingFallback({ label }: { label: string }) {
	return (
		<div className="flex h-full min-h-40 items-center justify-center text-sm text-muted-foreground">
			<span className="inline-flex items-center gap-2">
				<Spinner className="size-4" />
				{label}
			</span>
		</div>
	);
}
