<script lang="ts">
	import { Search } from "lucide-svelte";
	import { Badge, Button, Card, CardContent, Input } from "@/components/ui/svelte";
	import type { RbacInspectionSummary } from "@/lib/types";
	import { cnfast } from "@/lib/utils";
	import { filterCockpitItems, type RbacCockpitItem, type RbacRiskBucket } from "./cockpitModel";
	import type { RbacView } from "./surfaceModel";
	let { inspection, view, allItems, bucket, search = $bindable(), setView, setBucket }: {
		inspection: RbacInspectionSummary;
		view: RbacView;
		allItems: RbacCockpitItem[];
		bucket: RbacRiskBucket;
		search: string;
		setView: (view: RbacView) => void;
		setBucket: (bucket: RbacRiskBucket) => void;
	} = $props();

	const categories: RbacView[] = [
		"Namespace Access",
		"Roles",
		"Cluster Roles",
		"Bindings",
		"Service Accounts",
	];
	const riskFilters: Array<[RbacRiskBucket, string]> = [
		["all", "All"],
		["high", "High"],
		["medium", "Medium"],
		["low", "Low"],
		["none", "No flags"],
		["unknown", "Unknown"],
	];

	function categoryCount(inspection: RbacInspectionSummary, category: RbacView) {
		if (category === "Namespace Access") return inspection.namespaceAccess.length;
		if (category === "Roles") return inspection.roles.length;
		if (category === "Cluster Roles") return inspection.clusterRoles.length;
		if (category === "Bindings") {
			return inspection.roleBindings.length + inspection.clusterRoleBindings.length;
		}
		return inspection.serviceAccounts.length;
	}

	function riskCount(next: RbacRiskBucket) {
		return filterCockpitItems(allItems, next, "").length;
	}

</script>

<Card size="sm" elevation="flat" class="gap-0 py-0">
	<CardContent class="flex flex-col gap-2 px-2 py-2 xl:flex-row xl:items-center">
		<div class="flex min-w-0 items-center gap-1 overflow-x-auto" aria-label="RBAC categories">
			<span class="shrink-0 px-1 text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Explore</span>
			{#each categories as category}
				<Button
					size="sm"
					variant={view === category ? "secondary" : "ghost"}
					class={cnfast("shrink-0", view === category && "text-foreground")}
					aria-pressed={view === category}
					onclick={() => setView(category)}
				>
					{category}
					<Badge variant="outline" class="h-4 px-1.5 tabular-nums">{categoryCount(inspection, category)}</Badge>
				</Button>
			{/each}
		</div>
		<div class="flex min-w-0 flex-1 flex-wrap items-center gap-1 xl:justify-end">
			<span class="px-1 text-[0.625rem] font-semibold uppercase tracking-wide text-muted-foreground">Risk</span>
			{#each riskFilters as [riskBucket, label]}
				<Button
					size="xs"
					variant={bucket === riskBucket ? "default" : "outline"}
					aria-pressed={bucket === riskBucket}
					onclick={() => setBucket(riskBucket)}
				>
					{label} <span class="tabular-nums opacity-75">{riskCount(riskBucket)}</span>
				</Button>
			{/each}
			<div class="relative min-w-48 flex-1 xl:max-w-72">
				<Search class="pointer-events-none absolute left-2 top-1.5 size-3.5 text-muted-foreground" />
				<Input class="pl-7" bind:value={search} placeholder={`Search ${view.toLowerCase()}…`} />
			</div>
		</div>
	</CardContent>
</Card>
