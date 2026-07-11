<script lang="ts">
	import {
		Card,
		CardContent,
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow,
	} from "@/components/ui/svelte";

	let {
		headers,
		rows,
		empty,
	}: { headers: string[]; rows: string[][]; empty: string } = $props();
</script>

<Card size="sm" elevation="flat">
	<CardContent class="p-0">
		<Table class="min-w-[720px] table-fixed text-sm">
			<TableHeader>
				<TableRow>
					{#each headers as header}
						<TableHead
							class="px-3 py-2 text-xs font-semibold uppercase text-muted-foreground"
						>
							{header}
						</TableHead>
					{/each}
				</TableRow>
			</TableHeader>
			<TableBody>
				{#if rows.length === 0}
					<TableRow>
						<TableCell class="px-3 py-8 text-center text-muted-foreground" colspan={headers.length}>
							{empty}
						</TableCell>
					</TableRow>
				{:else}
					{#each rows as row}
						<TableRow>
							{#each row as cell}
								<TableCell class="truncate px-3 py-2">{cell}</TableCell>
							{/each}
						</TableRow>
					{/each}
				{/if}
			</TableBody>
		</Table>
	</CardContent>
</Card>
