<script lang="ts">
	import { tick } from "svelte";
	import { RotateCcw } from "lucide-svelte";
	import { Button } from "@/components/ui/svelte";
	import {
		defaultResourceKindLanes,
		OTHER_RESOURCE_KINDS,
		RESOURCE_KIND_LANE_IDS,
		type ResourceKindLaneId,
		type ResourceKindLanes,
	} from "@/lib/resource-kind-order";
	import { settingsStore } from "@/lib/settings-store";

	let { title, description }: { title: string; description: string } = $props();

	const LANE_TITLES: Record<ResourceKindLaneId, string> = {
		first: "Show first",
		normal: "Normal",
		last: "Show last",
	};

	let draft = $state<ResourceKindLanes | null>(null);
	let dragging = $state<string | null>(null);
	let container = $state<HTMLElement>();
	const lanes = $derived(draft ?? $settingsStore.resourceKindLanes);

	function kindLabel(kind: string): string {
		return kind === OTHER_RESOURCE_KINDS ? "All other kinds" : kind;
	}

	function laneOf(source: ResourceKindLanes, kind: string): ResourceKindLaneId {
		return RESOURCE_KIND_LANE_IDS.find((lane) => source[lane].includes(kind)) ?? "normal";
	}

	function moved(
		source: ResourceKindLanes,
		kind: string,
		lane: ResourceKindLaneId,
		index: number,
	): ResourceKindLanes {
		const next: ResourceKindLanes = {
			first: source.first.filter((entry) => entry !== kind),
			normal: source.normal.filter((entry) => entry !== kind),
			last: source.last.filter((entry) => entry !== kind),
		};
		next[lane].splice(Math.max(0, Math.min(index, next[lane].length)), 0, kind);
		return next;
	}

	// Pointer events instead of HTML5 drag events: Tauri's default file-drop
	// handler intercepts HTML5 drag events in WebView2 on Windows.
	function startDrag(event: PointerEvent & { currentTarget: HTMLElement }, kind: string) {
		if (event.button !== 0) return;
		event.preventDefault();
		event.currentTarget.focus();
		dragging = kind;
		draft = lanes;
		const move = (moveEvent: PointerEvent) => {
			const hit = document.elementFromPoint(moveEvent.clientX, moveEvent.clientY);
			const target = hit?.closest<HTMLElement>("[data-kind-drop]");
			if (!draft || !target || !container?.contains(target)) return;
			const drop = target.dataset.kindDrop ?? "";
			if (drop.startsWith("lane:")) {
				const lane = drop.slice(5) as ResourceKindLaneId;
				if (laneOf(draft, kind) !== lane) draft = moved(draft, kind, lane, draft[lane].length);
				return;
			}
			if (drop === kind) return;
			const lane = laneOf(draft, drop);
			draft = moved(draft, kind, lane, draft[lane].indexOf(drop));
		};
		const end = () => {
			window.removeEventListener("pointermove", move);
			window.removeEventListener("pointerup", end);
			window.removeEventListener("pointercancel", end);
			if (draft) $settingsStore.setResourceKindLanes(draft);
			draft = null;
			dragging = null;
		};
		window.addEventListener("pointermove", move);
		window.addEventListener("pointerup", end);
		window.addEventListener("pointercancel", end);
	}

	async function onKeydown(event: KeyboardEvent, kind: string) {
		const lane = laneOf(lanes, kind);
		const index = lanes[lane].indexOf(kind);
		const laneIndex = RESOURCE_KIND_LANE_IDS.indexOf(lane);
		let next: ResourceKindLanes | null = null;
		if (event.key === "ArrowUp" && index > 0) next = moved(lanes, kind, lane, index - 1);
		if (event.key === "ArrowDown" && index < lanes[lane].length - 1) {
			next = moved(lanes, kind, lane, index + 1);
		}
		const sideways = event.key === "ArrowLeft" ? -1 : event.key === "ArrowRight" ? 1 : 0;
		const targetLane = RESOURCE_KIND_LANE_IDS[laneIndex + sideways];
		if (sideways !== 0 && targetLane) next = moved(lanes, kind, targetLane, index);
		if (!next) return;
		event.preventDefault();
		$settingsStore.setResourceKindLanes(next);
		await tick();
		container
			?.querySelector<HTMLElement>(`[data-kind-drop="${CSS.escape(kind)}"]`)
			?.focus();
	}
</script>

<div class="rounded-md border bg-card p-3">
	<div class="mb-3 flex items-start justify-between gap-4">
		<div class="min-w-0 space-y-1">
			<div class="text-sm font-medium">{title}</div>
			<p class="text-sm text-muted-foreground">{description}</p>
		</div>
		<Button
			variant="ghost"
			size="sm"
			onclick={() => $settingsStore.setResourceKindLanes(defaultResourceKindLanes())}
		>
			<RotateCcw data-icon="inline-start" />Reset
		</Button>
	</div>
	<div bind:this={container} class="grid gap-2 md:grid-cols-3">
		{#each RESOURCE_KIND_LANE_IDS as lane (lane)}
			<section
				data-kind-drop="lane:{lane}"
				aria-label={LANE_TITLES[lane]}
				class="flex min-h-24 flex-col gap-1.5 rounded-md border bg-background p-2"
			>
				<span class="mb-1 text-sm font-medium">{LANE_TITLES[lane]}</span>
				<ol class="flex flex-col gap-1.5">
					{#each lanes[lane] as kind (kind)}
						<li>
							<button
								type="button"
								data-kind-drop={kind}
								aria-label="{kindLabel(kind)}, {LANE_TITLES[lane]}"
								class="w-full cursor-grab touch-none select-none rounded-md border px-2 py-1.5 text-left text-sm active:cursor-grabbing {dragging ===
								kind
									? 'border-primary bg-accent'
									: 'bg-card'} {kind === OTHER_RESOURCE_KINDS
									? 'border-dashed text-muted-foreground'
									: ''}"
								onpointerdown={(event) => startDrag(event, kind)}
								onkeydown={(event) => onKeydown(event, kind)}
							>
								{kindLabel(kind)}
							</button>
						</li>
					{/each}
				</ol>
			</section>
		{/each}
	</div>
</div>
