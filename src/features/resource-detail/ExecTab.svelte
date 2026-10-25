<script lang="ts">
	import "@xterm/xterm/css/xterm.css";

	import { onMount } from "svelte";
	import { ChevronDown, Play, Plus, RotateCcw, Square, TerminalSquare, X } from "lucide-svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Button,
		buttonClass,
		Empty,
		EmptyContent,
		EmptyDescription,
		EmptyHeader,
		EmptyMedia,
		EmptyTitle,
		Popover,
		PopoverContent,
		PopoverTrigger,
	} from "@/components/ui/svelte";
	import { podExecCommandLabel, podExecTarget } from "@/features/live-sessions";
	import { registerOpenExecSessions } from "@/features/live-sessions/openExecSessions.svelte";
	import { settingsStore } from "@/lib/settings-store";
	import type { TauriClient } from "@/lib/tauri";
	import type { PodExecSessionStatus, ResourceSummary } from "@/lib/types";
	import ExecSessionMenu, { type ExecMenuContainer } from "./ExecSessionMenu.svelte";
	import ExecSessionTerminal from "./ExecSessionTerminal.svelte";
	import type { ContainerStatusRow } from "./helpers";

	interface ExecSessionTab {
		key: number;
		container: string;
		command: string[];
		sessionId: string | null;
		status: PodExecSessionStatus | "stopped";
	}

	let {
		client,
		resource,
		containers,
		selectedContainer,
		kubeconfigSourceKey,
		active,
		onShow,
	}: {
		client: TauriClient;
		resource: ResourceSummary;
		containers: ContainerStatusRow[];
		selectedContainer: string;
		kubeconfigSourceKey?: string;
		active: boolean;
		onShow: () => void;
	} = $props();

	let tabs = $state<ExecSessionTab[]>([]);
	let activeKey = $state<number | null>(null);
	let menuOpen = $state(false);
	let nextKey = 0;
	const terminals: Record<number, ExecSessionTerminal> = {};

	const isPod = $derived(resource.kind === "Pod" && Boolean(resource.namespace));
	const activeTab = $derived(tabs.find((tab) => tab.key === activeKey) ?? null);
	const menuContainers = $derived.by((): ExecMenuContainer[] => {
		const appContainers = containers.filter((container) => container.type !== "init");
		const rows = appContainers.length > 0 ? appContainers : containers;
		return rows.length > 0
			? rows.map((container) => ({ name: container.name, restartCount: container.restartCount }))
			: [{ name: "", restartCount: 0 }];
	});
	const defaultContainer = $derived(
		menuContainers.some((container) => container.name === selectedContainer)
			? selectedContainer
			: menuContainers[0].name,
	);
	const sourceLabel = $derived(
		$settingsStore.showKubeconfigSourceLabels ? $settingsStore.kubeconfigSourceLabel || null : null,
	);

	onMount(() =>
		registerOpenExecSessions({
			sessions: () =>
				tabs.flatMap((tab) =>
					tab.sessionId
						? [{ id: tab.sessionId, podName: resource.name, container: tab.container, command: tab.command }]
						: [],
				),
			show: (sessionId) => {
				const tab = tabs.find((candidate) => candidate.sessionId === sessionId);
				if (!tab) return;
				activeKey = tab.key;
				onShow();
			},
			stopAll: async () => {
				const stopped = await Promise.all(tabs.map((tab) => terminals[tab.key]?.stop() ?? true));
				return stopped.every(Boolean);
			},
		}),
	);

	function openSession(container: string, command: string[]) {
		const tab: ExecSessionTab = { key: nextKey++, container, command, sessionId: null, status: "starting" };
		tabs.push(tab);
		activeKey = tab.key;
		menuOpen = false;
	}

	function closeTab(key: number) {
		const index = tabs.findIndex((tab) => tab.key === key);
		if (index === -1) return;
		tabs.splice(index, 1);
		delete terminals[key];
		if (activeKey === key) activeKey = (tabs[index] ?? tabs[index - 1])?.key ?? null;
	}

	function isLive(status: ExecSessionTab["status"]): boolean {
		return status === "starting" || status === "connecting" || status === "running";
	}

	function containerLabel(name: string): string {
		return name || "default container";
	}

	function tabLabel(tab: ExecSessionTab): string {
		return `${tab.command[0].split("/").pop()} · ${tab.container || "default"}`;
	}

	function statusDotClass(status: ExecSessionTab["status"]): string {
		if (status === "running") return "bg-emerald-500";
		if (status === "starting" || status === "connecting") return "bg-amber-500";
		if (status === "error") return "bg-red-500";
		return "bg-muted-foreground/60";
	}
</script>

{#snippet sessionMenu(align: "start" | "end")}
	<PopoverContent {align} class="w-64 p-1">
		<ExecSessionMenu
			podName={resource.name}
			containers={menuContainers}
			{defaultContainer}
			{sourceLabel}
			onOpen={openSession}
		/>
	</PopoverContent>
{/snippet}

{#if resource.kind !== "Pod"}
	<Empty class="min-h-32 border border-dashed">
		<EmptyHeader>
			<EmptyTitle>Exec starts from Pods</EmptyTitle>
			<EmptyDescription>Select an exact namespaced Pod before exec.</EmptyDescription>
		</EmptyHeader>
	</Empty>
{:else if !isPod}
	<Alert variant="destructive">
		<AlertTitle>Namespace required</AlertTitle>
		<AlertDescription>Pod exec requires a namespaced Pod target.</AlertDescription>
	</Alert>
{:else if tabs.length === 0}
	<Empty class="min-h-72 flex-1 border border-dashed">
		<EmptyHeader>
			<EmptyMedia variant="icon"><TerminalSquare /></EmptyMedia>
			<EmptyTitle>No exec sessions</EmptyTitle>
			<EmptyDescription>
				Open a shell inside this Pod. Sessions stop when you close the Pod or switch to another one.
			</EmptyDescription>
		</EmptyHeader>
		<EmptyContent>
			<div class="flex flex-wrap items-center justify-center gap-1.5">
				<Button onclick={() => openSession(defaultContainer, ["/bin/sh"])}>
					<TerminalSquare data-icon="inline-start" />
					Open /bin/sh in {containerLabel(defaultContainer)}
				</Button>
				<Popover bind:open={menuOpen}>
					<PopoverTrigger class={buttonClass({ variant: "outline" })}>
						Other
						<ChevronDown data-icon="inline-end" />
					</PopoverTrigger>
					{@render sessionMenu("end")}
				</Popover>
			</div>
		</EmptyContent>
	</Empty>
{:else}
	<div class="flex min-h-72 flex-1 flex-col overflow-hidden rounded-lg border bg-card">
		<div class="flex h-9 shrink-0 items-stretch gap-0.5 border-b pr-1.5 pl-1">
			<div
				role="group"
				aria-label="Exec sessions"
				class="flex min-w-0 items-stretch overflow-x-auto overflow-y-hidden [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
			>
				{#each tabs as tab (tab.key)}
					{@const selected = tab.key === activeKey}
					<div
						class="group/session relative flex shrink-0 items-center gap-1 pr-1 pl-2 text-xs font-medium {selected
							? 'text-foreground after:absolute after:inset-x-1.5 after:bottom-0 after:h-0.5 after:rounded-full after:bg-foreground'
							: 'text-muted-foreground hover:text-foreground'}"
					>
						<button
							type="button"
							aria-pressed={selected}
							aria-label={`${tabLabel(tab)}, ${tab.status}`}
							class="flex cursor-pointer items-center gap-1.5 rounded-sm whitespace-nowrap outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
							title={`${podExecCommandLabel(tab.command)} on ${podExecTarget(resource, tab.container)}`}
							onclick={() => (activeKey = tab.key)}
						>
							<span class="size-[7px] shrink-0 rounded-full {statusDotClass(tab.status)}"></span>
							{tabLabel(tab)}
						</button>
						<Button
							variant="ghost"
							size="icon-xs"
							class="text-muted-foreground {selected ? '' : 'opacity-0 group-hover/session:opacity-100 focus-visible:opacity-100'}"
							aria-label={`${isLive(tab.status) ? "Stop and close" : "Close"} ${tabLabel(tab)}`}
							title={isLive(tab.status) ? "Stop and close" : "Close"}
							onclick={() => closeTab(tab.key)}
						>
							<X />
						</Button>
					</div>
				{/each}
			</div>
			<Popover bind:open={menuOpen}>
				<PopoverTrigger
					class={buttonClass({ variant: "ghost", size: "icon-sm", className: "self-center text-muted-foreground" })}
					aria-label="New exec session"
					title="New exec session"
				>
					<Plus />
				</PopoverTrigger>
				{@render sessionMenu("start")}
			</Popover>
			<span class="flex-1"></span>
			{#if activeTab}
				<div class="flex shrink-0 items-center gap-1">
					<span class="mr-1 inline-flex items-center gap-1.5 text-xs whitespace-nowrap">
						<span class="size-[7px] shrink-0 rounded-full {statusDotClass(activeTab.status)}"></span>
						{activeTab.status}
					</span>
					<Button
						variant="ghost"
						size="icon-sm"
						class="text-muted-foreground"
						aria-label="Clear terminal"
						title="Clear terminal"
						onclick={() => terminals[activeTab.key]?.clear()}
					>
						<RotateCcw />
					</Button>
					{#if isLive(activeTab.status)}
						<Button
							variant="ghost"
							size="icon-sm"
							class="text-destructive hover:text-destructive"
							aria-label="Stop session"
							title="Stop session"
							onclick={() => void terminals[activeTab.key]?.stop()}
						>
							<Square />
						</Button>
					{:else}
						<Button
							variant="ghost"
							size="icon-sm"
							class="text-muted-foreground"
							aria-label={`Restart ${podExecCommandLabel(activeTab.command)} in ${containerLabel(activeTab.container)}`}
							title="Restart session"
							onclick={() => terminals[activeTab.key]?.restart()}
						>
							<Play />
						</Button>
					{/if}
				</div>
			{/if}
		</div>
		<div class="relative min-h-0 flex-1 bg-[#101113]">
			{#each tabs as tab (tab.key)}
				<ExecSessionTerminal
					bind:this={terminals[tab.key]}
					bind:sessionId={tab.sessionId}
					bind:status={tab.status}
					{client}
					{resource}
					{kubeconfigSourceKey}
					container={tab.container}
					command={tab.command}
					visible={active && tab.key === activeKey}
				/>
			{/each}
		</div>
	</div>
{/if}
