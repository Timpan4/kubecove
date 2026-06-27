<script lang="ts">
	import { Card, CardContent } from "@/components/ui/svelte";

	let {
		headers,
		rows,
		empty,
	}: { headers: string[]; rows: string[][]; empty: string } = $props();
</script>

<Card size="sm" elevation="flat">
	<CardContent class="overflow-x-auto p-0">
		<table class="w-full min-w-[720px] table-fixed border-collapse text-sm">
			<thead>
				<tr>
					{#each headers as header}
						<th
							class="border-b px-3 py-2 text-left text-xs font-semibold uppercase text-muted-foreground"
						>
							{header}
						</th>
					{/each}
				</tr>
			</thead>
			<tbody>
				{#if rows.length === 0}
					<tr>
						<td class="px-3 py-8 text-center text-muted-foreground" colspan={headers.length}>
							{empty}
						</td>
					</tr>
				{:else}
					{#each rows as row}
						<tr class="border-b last:border-b-0">
							{#each row as cell}
								<td class="truncate px-3 py-2">{cell}</td>
							{/each}
						</tr>
					{/each}
				{/if}
			</tbody>
		</table>
	</CardContent>
</Card>
