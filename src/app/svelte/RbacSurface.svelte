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

	let { rbacQuery, rbacStats, rbacTable, rbacView, rbacWarningSummary } = $props();
</script>

<SurfaceFrame icon={KeyRound} title="RBAC" query={rbacQuery} errorLabel="RBAC inspection unavailable">
		{@const data = rbacQuery.data}
		{#if data}
			<StatGrid stats={rbacStats} />
			{#if rbacTable}
				<Card size="sm" elevation="flat">
					<CardHeader>
						<CardTitle>{rbacView}</CardTitle>
						<CardDescription>Read-only RBAC inspection for current workspace scope.</CardDescription>
					</CardHeader>
					<CardContent>
							<SimpleTable
								headers={rbacTable.headers}
								rows={rbacTable.rows}
								empty={rbacTable.empty}
							/>
					</CardContent>
				</Card>
			{/if}
			{#if data.warnings.length > 0}
				<FriendlyError
					mode="compact"
					error={rbacWarningSummary(data.warnings)}
					context={{
						operation: "resourcesLoad",
						fallbackTitle: "Partial RBAC data",
						partial: true,
					}}
				/>
			{/if}
		{/if}
	</SurfaceFrame>
