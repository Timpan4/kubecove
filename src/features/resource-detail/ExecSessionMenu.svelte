<script module lang="ts">
	export interface ExecMenuContainer {
		/** Empty means Kubernetes picks the default container. */
		name: string;
		restartCount: number;
	}
</script>

<script lang="ts">
	import { Pencil, TerminalSquare } from "lucide-svelte";
	import { untrack } from "svelte";
	import {
		Badge,
		Button,
		Select,
		SelectContent,
		SelectGroup,
		SelectItem,
		SelectTrigger,
		SelectValue,
		Textarea,
	} from "@/components/ui/svelte";
	import { parsePodExecArgv, POD_EXEC_SHELLS, podExecCommandLabel } from "@/features/live-sessions";

	let {
		podName,
		containers,
		defaultContainer,
		sourceLabel,
		onOpen,
	}: {
		podName: string;
		containers: ExecMenuContainer[];
		defaultContainer: string;
		sourceLabel: string | null;
		onOpen: (container: string, command: string[]) => void;
	} = $props();

	let mode = $state<"shells" | "custom">("shells");
	let customContainer = $state(untrack(() => defaultContainer));
	let argvText = $state("");
	const argv = $derived(parsePodExecArgv(argvText));

	const itemClass =
		"flex h-7 w-full cursor-pointer items-center gap-2 rounded-sm px-2 text-left text-xs outline-none hover:bg-muted focus-visible:bg-muted [&_svg]:size-3.5 [&_svg]:shrink-0 [&_svg]:text-muted-foreground";

	function containerLabel(name: string): string {
		return name || "default container";
	}

	function runCustom() {
		if (argv.length > 0) onOpen(customContainer, argv);
	}
</script>

{#if mode === "shells"}
	<div class="truncate px-2 pt-1.5 pb-1 text-[11px] text-muted-foreground">
		Open in <span class="font-mono">{podName}</span>{#if sourceLabel} · {sourceLabel}{/if}
	</div>
	<div role="menu" aria-label="New exec session">
		{#each containers as container (container.name)}
			<div class="flex items-center gap-1.5 px-2 pt-1.5 pb-0.5 text-[11px] font-medium text-muted-foreground">
				<span class="truncate">{containerLabel(container.name)}</span>
				{#if container.restartCount > 0}
					<Badge variant="outline">{container.restartCount} {container.restartCount === 1 ? "restart" : "restarts"}</Badge>
				{/if}
			</div>
			{#each POD_EXEC_SHELLS as shell (shell[0])}
				<button
					type="button"
					role="menuitem"
					class={itemClass}
					title={`${podExecCommandLabel(shell)} in ${containerLabel(container.name)}`}
					onclick={() => onOpen(container.name, [...shell])}
				>
					<TerminalSquare />
					<span class="font-mono">{podExecCommandLabel(shell)}</span>
				</button>
			{/each}
		{/each}
		<div class="-mx-1 my-1 h-px bg-border"></div>
		<button type="button" role="menuitem" class={itemClass} onclick={() => (mode = "custom")}>
			<Pencil />
			Custom command…
		</button>
	</div>
{:else}
	<div class="flex flex-col gap-2.5 p-1.5">
		<div class="truncate text-[11px] text-muted-foreground">
			Custom command in <span class="font-mono">{podName}</span>
		</div>
		{#if containers.length > 1}
			<label class="flex flex-col gap-1 text-[11px] font-medium text-muted-foreground">
				Container
				<Select
					value={customContainer}
					items={containers.map((container) => ({ value: container.name, label: containerLabel(container.name) }))}
					onValueChange={(value: string) => (customContainer = value)}
				>
					<SelectTrigger class="h-7 w-full" aria-label="Exec container">
						<SelectValue />
					</SelectTrigger>
					<SelectContent>
						<SelectGroup>
							{#each containers as container (container.name)}
								<SelectItem value={container.name}>{containerLabel(container.name)}</SelectItem>
							{/each}
						</SelectGroup>
					</SelectContent>
				</Select>
			</label>
		{/if}
		<label class="flex flex-col gap-1 text-[11px] font-medium text-muted-foreground">
			Arguments, one per line
			<Textarea
				bind:value={argvText}
				rows={3}
				class="font-mono text-xs"
				placeholder={"/usr/bin/env\nprintenv"}
				onkeydown={(event: KeyboardEvent) => {
					if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
						event.preventDefault();
						runCustom();
					}
				}}
			/>
			<span class="font-normal">Passed as-is. No shell parsing.</span>
		</label>
		<div class="flex items-center justify-between gap-2">
			<Button variant="ghost" size="sm" onclick={() => (mode = "shells")}>Back</Button>
			<Button size="sm" disabled={argv.length === 0} onclick={runCustom} class="min-w-0">
				<span class="truncate">
					Run {argv.length > 0 ? argv[0].split("/").pop() : "command"} in {containerLabel(customContainer)}
				</span>
			</Button>
		</div>
	</div>
{/if}
