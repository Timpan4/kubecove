<script lang="ts">
	import { ChevronDown, Search } from "lucide-svelte";
	import {
		Button,
		Checkbox,
		InputGroup,
		InputGroupAddon,
		InputGroupInput,
		InputGroupText,
		Label,
		Popover,
		PopoverContent,
		PopoverTrigger,
		ScrollArea,
	} from "@/components/ui/svelte";
	import {
		filterResourceScopeOptions,
		type ResourceScopeOption,
	} from "./resourceBrowserModel";

	let {
		triggerLabel,
		triggerValue,
		triggerAriaLabel,
		heading,
		selectAllLabel,
		searchAriaLabel,
		searchPlaceholder,
		emptyLabel,
		noMatchesLabel,
		options,
		selectedKeys,
		allSelected,
		onSelectAll,
		onToggle,
	}: {
		triggerLabel: string;
		triggerValue: string;
		triggerAriaLabel: string;
		heading: string;
		selectAllLabel: string;
		searchAriaLabel: string;
		searchPlaceholder: string;
		emptyLabel: string;
		noMatchesLabel: string;
		options: ResourceScopeOption[];
		selectedKeys: Set<string>;
		allSelected: boolean;
		onSelectAll: () => void;
		onToggle: (key: string, checked: boolean) => void;
	} = $props();

	let search = $state("");
	let open = $state(false);
	const filteredOptions = $derived(filterResourceScopeOptions(options, search));
</script>

<Popover bind:open>
	<PopoverTrigger
		type="button"
		class="inline-flex h-8 min-w-0 max-w-full cursor-pointer items-center gap-1.5 rounded-md border bg-background/70 px-2.5 text-xs text-muted-foreground hover:bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
		aria-label={triggerAriaLabel}
	>
		{triggerLabel}
		<strong class="min-w-0 truncate font-semibold text-foreground">{triggerValue}</strong>
		<ChevronDown class="size-3.5 shrink-0" />
	</PopoverTrigger>
	<PopoverContent align="start" class="w-[min(22rem,calc(100vw-1rem))] p-2">
		<div class="mb-2 px-1 text-[0.625rem] font-semibold uppercase text-muted-foreground">
			{heading}
		</div>
		<Button
			variant="outline"
			size="sm"
			class="h-8 w-full justify-start"
			disabled={allSelected}
			onclick={onSelectAll}
		>
			{selectAllLabel}
		</Button>
		<InputGroup class="mt-1 h-8 border bg-background/70">
			<InputGroupAddon align="inline-start">
				<InputGroupText><Search class="size-4" /></InputGroupText>
			</InputGroupAddon>
			<InputGroupInput
				aria-label={searchAriaLabel}
				class="h-7 text-xs"
				bind:value={search}
				placeholder={searchPlaceholder}
			/>
		</InputGroup>
		<ScrollArea class="mt-2 h-56 rounded-md border bg-background/30">
			<div class="flex flex-col gap-1 p-1">
				{#each filteredOptions as option (option.key)}
					<Label class="cursor-pointer rounded-md px-2 py-1.5 hover:bg-muted/50">
						<Checkbox
							checked={allSelected || selectedKeys.has(option.key)}
							onCheckedChange={(checked) => onToggle(option.key, checked)}
						/>
						<span class="truncate">{option.label}</span>
					</Label>
				{:else}
					<div class="px-2 py-1.5 text-xs text-muted-foreground">
						{options.length > 0 ? noMatchesLabel : emptyLabel}
					</div>
				{/each}
			</div>
		</ScrollArea>
	</PopoverContent>
</Popover>
