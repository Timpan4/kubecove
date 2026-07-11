<script lang="ts">
	import { KeyRound } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle,
	} from "@/components/ui/svelte";
	import SimpleTable from "@/components/SimpleTable.svelte";
	import StatGrid from "@/components/StatGrid.svelte";
	import SurfaceFrame from "@/components/SurfaceFrame.svelte";
	import type { RbacInspectionSummary } from "@/lib/types";
	import type { RbacTable, RbacView } from "./surfaceModel";

	let { query, stats, table, view, warningSummary }: {
		query: {
			data?: RbacInspectionSummary;
			isPending: boolean;
			isError: boolean;
			error: unknown;
		};
		stats: Array<[string, number]>;
		table: RbacTable | null;
		view: RbacView;
		warningSummary: (warnings: string[]) => string;
	} = $props();
</script>

<SurfaceFrame icon={KeyRound} title="RBAC" {query} errorLabel="RBAC inspection unavailable">
		{@const data = query.data}
		{#if data}
			<StatGrid {stats} />
			{#if table}
				<Card size="sm" elevation="flat">
					<CardHeader>
						<CardTitle>{view}</CardTitle>
						<CardDescription>Read-only RBAC inspection for current workspace scope.</CardDescription>
					</CardHeader>
					<CardContent>
							<SimpleTable
								headers={table.headers}
								rows={table.rows}
								empty={table.empty}
							/>
					</CardContent>
				</Card>
			{/if}
			{#if data.warnings.length > 0}
				<FriendlyError
					mode="compact"
					error={warningSummary(data.warnings)}
					context={{
						operation: "resourcesLoad",
						fallbackTitle: "Partial RBAC data",
						partial: true,
					}}
				/>
			{/if}
		{/if}
	</SurfaceFrame>
