<script lang="ts">
	import { CheckCircle2, Info, TriangleAlert, X, XCircle } from "lucide-svelte";
	import { Button } from "@/components/ui/svelte";
	import { dismissToast, toasts, type ToastTone } from "@/lib/toasts";

	function toneClass(tone: ToastTone): string {
		if (tone === "success") return "border-emerald-500/35 bg-emerald-950/90";
		if (tone === "warning") return "border-amber-500/35 bg-amber-950/90";
		if (tone === "error") return "border-destructive/45 bg-destructive/15";
		return "border-border bg-popover/95";
	}
</script>

<div
	class="pointer-events-none fixed right-4 top-14 z-[var(--z-toast)] flex w-[22rem] max-w-[calc(100vw-2rem)] flex-col gap-2"
	aria-live="polite"
	aria-label="Notifications"
>
	{#each $toasts as toast (toast.id)}
		<div
			class={`pointer-events-auto flex items-start gap-2 rounded-lg border p-3 text-sm shadow-lg backdrop-blur-md ${toneClass(toast.tone)}`}
			role={toast.tone === "error" ? "alert" : "status"}
		>
			{#if toast.tone === "success"}
				<CheckCircle2 class="mt-0.5 size-4 shrink-0 text-emerald-400" />
			{:else if toast.tone === "warning"}
				<TriangleAlert class="mt-0.5 size-4 shrink-0 text-amber-400" />
			{:else if toast.tone === "error"}
				<XCircle class="mt-0.5 size-4 shrink-0 text-destructive" />
			{:else}
				<Info class="mt-0.5 size-4 shrink-0 text-primary" />
			{/if}
			<div class="min-w-0 flex-1">
				<div class="font-medium text-foreground">{toast.title}</div>
				{#if toast.description}
					<div class="mt-0.5 text-xs leading-relaxed text-muted-foreground">{toast.description}</div>
				{/if}
			</div>
			<Button
				type="button"
				variant="ghost"
				size="icon-xs"
				class="-mr-1 -mt-1 shrink-0 text-muted-foreground"
				aria-label="Dismiss notification"
				onclick={() => dismissToast(toast.id)}
			>
				<X />
			</Button>
		</div>
	{/each}
</div>
