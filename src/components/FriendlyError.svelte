<script lang="ts">
	import { Check, CircleAlert, Copy, KeyRound } from "lucide-svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Button,
	} from "@/components/ui/svelte";
	import {
		friendlyError,
		requiredPermissionForFriendlyError,
		type FriendlyErrorContext,
		type FriendlyErrorMode,
	} from "@/lib/friendly-errors";
import { openRbacVerifier } from "@/features/rbac";
	import { cnfast } from "@/lib/utils";

	let {
		error,
		mode = "full",
		context = {},
		class: className = "",
	}: {
		error: unknown;
		mode?: FriendlyErrorMode;
		context?: FriendlyErrorContext;
		class?: string;
	} = $props();

	let copied = $state(false);
	let copyFailed = $state(false);
	const presentation = $derived(friendlyError(error, context));
	const compact = $derived(mode === "compact");
	const requiredPermission = $derived(requiredPermissionForFriendlyError(error, context));
	const alertClass = $derived(
		cnfast(
			presentation.tone === "warning"
				? "border-amber-500/35 bg-amber-500/5 text-foreground"
				: "border-destructive/30 bg-destructive/5",
			compact ? "py-2" : "p-3",
			className,
		),
	);

	async function copyDetail() {
		try {
			await navigator.clipboard.writeText(presentation.copyText);
			copied = true;
			copyFailed = false;
			window.setTimeout(() => {
				copied = false;
			}, 1200);
		} catch {
			copyFailed = true;
		}
	}

	function inspectRequiredPermission() {
		if (!requiredPermission) return;
		openRbacVerifier({
			target: requiredPermission,
			sourceLabel: context.permissionSourceLabel ?? presentation.title,
		});
	}
</script>

<Alert variant={presentation.tone === "destructive" ? "destructive" : "default"} class={alertClass}>
	<CircleAlert
		class={cnfast(
			"size-4",
			presentation.tone === "warning" ? "text-amber-500" : "text-destructive",
		)}
	/>
	<AlertTitle class={cnfast("text-sm", compact && "text-xs")}>
		{presentation.title}
	</AlertTitle>
	<AlertDescription class="space-y-2 text-xs/relaxed text-foreground/80">
		<p>{presentation.summary}</p>
		{#if !compact && presentation.stillWorks}
			<p>{presentation.stillWorks}</p>
		{/if}
		{#if !compact && presentation.next}
			<p>{presentation.next}</p>
		{/if}
		{#if requiredPermission}
			<Button type="button" variant="outline" size="xs" onclick={inspectRequiredPermission}>
				<KeyRound data-icon="inline-start" />
				Inspect required permission
			</Button>
		{/if}
		<details class="rounded-md border bg-background/55 px-2 py-1.5">
			<summary class="cursor-pointer text-xs font-medium text-muted-foreground">
				Technical detail
			</summary>
			<div class="mt-2 space-y-2">
				<pre class="max-h-40 overflow-auto whitespace-pre-wrap break-words rounded bg-muted/40 p-2 text-[0.6875rem] leading-relaxed text-muted-foreground">{presentation.technicalDetail}</pre>
				<div class="flex items-center gap-2">
					<Button type="button" variant="outline" size="xs" onclick={copyDetail}>
						{#if copied}
							<Check data-icon="inline-start" />
							Copied
						{:else}
							<Copy data-icon="inline-start" />
							Copy detail
						{/if}
					</Button>
					{#if copyFailed}
						<span class="text-[0.6875rem] text-muted-foreground">Copy failed</span>
					{/if}
				</div>
			</div>
		</details>
	</AlertDescription>
</Alert>
