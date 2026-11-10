<script lang="ts">
	import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/svelte";

	// The ownership map shows one context at a time; ownership edges never cross clusters.
	let { contexts, value, onChange }: {
		contexts: readonly string[];
		value: string;
		onChange: (context: string) => void;
	} = $props();
</script>

<Select
	{value}
	items={contexts.map((context) => ({ value: context, label: context }))}
	onValueChange={(next: string) => {
		if (next) onChange(next);
	}}
>
	<SelectTrigger ariaLabel="Map context" class="h-8 max-w-48 border bg-background/70">
		<SelectValue placeholder="Map context" />
	</SelectTrigger>
	<SelectContent>
		<SelectGroup>
			{#each contexts as context (context)}
				<SelectItem value={context}>{context}</SelectItem>
			{/each}
		</SelectGroup>
	</SelectContent>
</Select>
