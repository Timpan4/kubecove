<script lang="ts">
	import { ChevronRight } from "lucide-svelte";
	import { Button, Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/svelte";
	import { resourceKey } from "@/lib/resource-identity";
	import { PAGE_SIZE } from "@/features/resources/constants";
	import type { ResourceSummary } from "@/lib/types";
	import type { Problem } from "./model";
	import ProblemStatus from "./ProblemStatus.svelte";
	import { DOT, LINK, plural, type OverviewView } from "./view";
	let { view, problems }: { view: OverviewView; problems: Problem[] } = $props();
	let expanded = $state<Record<string, boolean>>({});
	let page = $state(0);
	const pages = $derived(Math.max(1, Math.ceil(problems.length / PAGE_SIZE)));
	const currentPage = $derived(Math.min(page, pages - 1));
	const visible = $derived(problems.slice(currentPage * PAGE_SIZE, (currentPage + 1) * PAGE_SIZE));
	const label = (row: ResourceSummary) => `Open ${row.kind} ${row.name} in ${row.namespace ?? "cluster scope"} on ${row.cluster}`;
</script>
<Table aria-label="Workspace findings" class="min-w-[40rem] table-fixed">
	<colgroup>
		<col class="w-[34%]" /><col class="w-[33%]" /><col class="w-[9%]" /><col class="w-[11%]" /><col class="w-[13%]" />
	</colgroup>
	<TableHeader>
		<TableRow class="hover:bg-transparent">
			<TableHead class="pl-3 text-muted-foreground">Workload / resource</TableHead>
			<TableHead class="text-muted-foreground">Status</TableHead>
			<TableHead class="text-right text-muted-foreground">Ready</TableHead>
			<TableHead class="pr-5 text-right text-muted-foreground">Restarts</TableHead>
			<TableHead class="pr-3 text-muted-foreground">GitOps app</TableHead>
		</TableRow>
	</TableHeader>
	<TableBody>
		{#each visible as problem (problem.key)}
			{@const open = expanded[problem.key] === true}
			{@const owner = problem.resource.gitOpsOwner}
			{@const app = owner?.name ?? problem.resource.argoApp}
			<TableRow data-overview-resource={problem.key}>
				<TableCell class="pl-3 align-top whitespace-normal">
					<div class="flex items-start gap-2">
						<span class={["mt-1.5 size-2 shrink-0 rounded-full", DOT[problem.tone]]} aria-hidden="true"></span>
						<div class="grid min-w-0 justify-items-start gap-0.5">
							<Button variant="link" class={LINK} aria-label={label(problem.resource)} onclick={() => view.onOpenResource(problem.resource)}>{problem.resource.name}</Button>
							<span class="text-muted-foreground">{problem.resource.kind} in {problem.resource.namespace ?? "cluster scope"}{view.multiContext ? ` on ${problem.resource.cluster}` : ""}</span>
							{#if problem.related.length > 0}
								<Button variant="ghost" size="sm" class="-ml-2 text-muted-foreground"
									aria-expanded={open} aria-label={`Owned resources of ${problem.resource.name} in ${problem.resource.namespace} on ${problem.resource.cluster}`}
									onclick={() => expanded[problem.key] = !open}>
									<ChevronRight class={`transition-transform duration-150 motion-reduce:transition-none ${open ? "rotate-90" : ""}`} />
									{plural(problem.related.length, "owned resource")} also failing
								</Button>
							{/if}
						</div>
					</div>
				</TableCell>
				<TableCell class="align-top whitespace-normal">
					<ProblemStatus resource={problem.cause} tone={problem.tone} />
					{#if problem.cause !== problem.resource}
						<div class="mt-1 flex flex-wrap items-baseline gap-x-1 text-muted-foreground">
							<span>From {problem.cause.kind}</span>
							<Button variant="link" class={LINK} aria-label={label(problem.cause)} onclick={() => view.onOpenResource(problem.cause)}>{problem.cause.name}</Button>
						</div>
					{/if}
				</TableCell>
				<TableCell class="text-right align-top tabular-nums">{problem.resource.ready ?? "-"}</TableCell>
				<TableCell class="pr-5 text-right align-top tabular-nums">{problem.restarts}</TableCell>
				<TableCell class="pr-3 align-top whitespace-normal">
					{#if app && owner?.provider === "argo" && owner.kind === "Application" && owner.namespace}
						<Button variant="link" class={LINK} onclick={() => view.onOpenApp(problem.resource)}>{app}</Button>
					{:else}<span class="font-mono break-all text-muted-foreground">{app ?? "None"}</span>{/if}
				</TableCell>
			</TableRow>
			{#if open}
				{#each problem.related as member (resourceKey(member.resource))}
					<TableRow class="bg-muted/20 animate-in fade-in duration-150 motion-reduce:animate-none" data-overview-resource={resourceKey(member.resource)}>
						<TableCell class="pl-8 align-top whitespace-normal">
							<div class="grid min-w-0 justify-items-start gap-0.5">
								<Button variant="link" class={LINK} aria-label={label(member.resource)} onclick={() => view.onOpenResource(member.resource)}>{member.resource.name}</Button>
								<span class="text-muted-foreground">{member.resource.kind}</span>
							</div>
						</TableCell>
						<TableCell class="align-top whitespace-normal"><ProblemStatus resource={member.resource} tone={member.tone} /></TableCell>
						<TableCell class="text-right align-top tabular-nums text-muted-foreground">{member.resource.ready ?? "-"}</TableCell>
						<TableCell class="pr-5 text-right align-top tabular-nums text-muted-foreground">{member.resource.restarts ?? 0}</TableCell>
						<TableCell></TableCell>
					</TableRow>
				{/each}
			{/if}
		{/each}
	</TableBody>
</Table>
{#if pages > 1}
	<nav aria-label="Findings pages" class="flex items-center justify-end gap-2 border-t px-3 pt-3">
		<span class="text-xs text-muted-foreground">Page {currentPage + 1} of {pages}</span>
		<Button variant="outline" size="sm" disabled={currentPage === 0} onclick={() => page = currentPage - 1}>Previous findings</Button>
		<Button variant="outline" size="sm" disabled={currentPage === pages - 1} onclick={() => page = currentPage + 1}>Next findings</Button>
	</nav>
{/if}
