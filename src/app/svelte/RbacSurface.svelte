<script lang="ts">
	import { KeyRound } from "lucide-svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle,
	} from "@/components/ui/svelte";
	import SimpleTable from "./SimpleTable.svelte";
	import StatGrid from "./StatGrid.svelte";
	import SurfaceFrame from "./SurfaceFrame.svelte";

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
				<Alert><AlertTitle>Partial RBAC data</AlertTitle><AlertDescription>{rbacWarningSummary(data.warnings)}</AlertDescription></Alert>
			{/if}
		{/if}
	</SurfaceFrame>
