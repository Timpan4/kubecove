<script lang="ts">
	import { Square } from "lucide-svelte";
	import {
		Button,
		Dialog,
		DialogContent,
		DialogDescription,
		DialogHeader,
		DialogTitle,
	} from "@/components/ui/svelte";
	import { answerStopPrompt, pendingStopPrompt } from "./openExecSessions.svelte";
	import { podExecCommandLabel } from "./podExecHelpers";

	const prompt = $derived(pendingStopPrompt.current);
	const count = $derived(prompt?.sessions.length ?? 0);
	const podNames = $derived([...new Set(prompt?.sessions.map((session) => session.podName))].join(", "));
</script>

<Dialog
	open={prompt !== null}
	onOpenChange={(open: boolean) => {
		if (!open) answerStopPrompt(false);
	}}
>
	<DialogContent class="max-w-sm">
		<DialogHeader>
			<DialogTitle>Stop {count} exec {count === 1 ? "session" : "sessions"}?</DialogTitle>
			<DialogDescription>
				Leaving <span class="font-mono">{podNames}</span> stops {count === 1 ? "this session" : "these sessions"}. Terminal output is not kept.
			</DialogDescription>
		</DialogHeader>
		<ul class="flex flex-col gap-1.5 text-xs">
			{#each prompt?.sessions ?? [] as session (session.id)}
				<li class="flex min-w-0 items-center gap-2">
					<span class="size-[7px] shrink-0 rounded-full bg-emerald-500"></span>
					<span class="truncate font-mono">{podExecCommandLabel(session.command)}</span>
					<span class="shrink-0 text-muted-foreground">in {session.container || "default container"}</span>
				</li>
			{/each}
		</ul>
		<div class="flex justify-end gap-2">
			<Button variant="outline" onclick={() => answerStopPrompt(false)}>Stay</Button>
			<Button variant="destructive" onclick={() => answerStopPrompt(true)}>
				<Square data-icon="inline-start" />
				Stop and leave
			</Button>
		</div>
	</DialogContent>
</Dialog>
