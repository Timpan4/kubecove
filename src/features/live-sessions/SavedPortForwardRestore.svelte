<script lang="ts">
	import { useQueryClient } from "@tanstack/svelte-query";
	import { Cable, Eye, Play, X } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Button,
		Checkbox,
		Field,
		FieldLabel,
	} from "@/components/ui/svelte";
	import { workspaceStore } from "@/features/workspaces/workspaceStore";
	import { getSettingsSnapshot, settingsStore } from "@/lib/settings-store";
	import type { TauriClient } from "@/lib/tauri";
	import type { SavedWorkspace } from "@/lib/workspace-model";
	import {
		savedPortForwardStartFailureMessage,
		shouldAutoStartSavedPortForwards,
		shouldShowSavedPortForwardRestorePrompt,
		startSavedPortForwards,
	} from "./portForwardLifecycle";

	let {
		client,
		workspace,
		onReview,
	}: {
		client: TauriClient;
		workspace: SavedWorkspace;
		onReview: () => void;
	} = $props();

	const queryClient = useQueryClient();
	let dismissedWorkspaceId = $state<string | null>(null);
	let starting = $state(false);
	let restoreError = $state<unknown>(null);
	const autoStartedWorkspaceIds = new Set<string>();
	const autoStart = $derived($settingsStore.autoStartSavedPortForwards);
	const kubeconfigSourceKey = $derived($settingsStore.kubeconfigSourceKey);
	const savedCount = $derived(workspace.portForwards?.length ?? 0);
	const showPrompt = $derived(
		shouldShowSavedPortForwardRestorePrompt({
			workspace,
			autoStart,
			dismissedWorkspaceId,
		}),
	);

	$effect(() => {
		if (
			!shouldAutoStartSavedPortForwards({
				workspace,
				autoStart,
				startedWorkspaceIds: autoStartedWorkspaceIds,
			})
		) {
			return;
		}
		autoStartedWorkspaceIds.add(workspace.id);
		void startAll(false);
	});

	function dismiss() {
		dismissedWorkspaceId = workspace.id;
		restoreError = null;
	}

	async function startAll(dismissOnSuccess: boolean) {
		starting = true;
		restoreError = null;
		try {
			const results = await startSavedPortForwards({
				client,
				workspace,
				kubeconfigSource: kubeconfigSourceKey,
				updateSavedPortForward: workspaceStore.updateSavedPortForward,
				invalidateQueries: (options) => queryClient.invalidateQueries(options),
			});
			const failureMessage = savedPortForwardStartFailureMessage(results);
			if (failureMessage) {
				restoreError = failureMessage;
				return;
			}
			if (dismissOnSuccess) dismiss();
		} catch (error) {
			restoreError = error;
		} finally {
			starting = false;
		}
	}
</script>

{#if showPrompt}
	<Alert class="shrink-0 rounded-none border-x-0 border-t-0">
		<Cable class="size-3.5" />
		<AlertTitle>Saved port forwards available</AlertTitle>
		<AlertDescription class="flex flex-wrap items-center justify-between gap-3">
			<span>
				This workspace has {savedCount} saved Service{" "}
				{savedCount === 1 ? "forward" : "forwards"} ready to start.
			</span>
			{#if restoreError}
				<div class="w-full">
					<FriendlyError
						mode="compact"
						error={restoreError}
						context={{
							operation: "portForward",
							fallbackTitle: "Saved port-forwards failed",
							partial: true,
						}}
					/>
				</div>
			{/if}
			<span class="flex flex-wrap items-center gap-2">
				<Field orientation="horizontal" class="w-auto pr-2">
					<FieldLabel class="items-center gap-2 text-xs text-muted-foreground">
						<Checkbox
							checked={autoStart}
							onCheckedChange={(checked: boolean) =>
								getSettingsSnapshot().setAutoStartSavedPortForwards(checked)}
							aria-label="Auto-start saved port forwards"
						/>
						Auto-start
					</FieldLabel>
				</Field>
				<Button
					type="button"
					size="sm"
					disabled={starting}
					onclick={() => void startAll(true)}
				>
					<Play data-icon="inline-start" />
					Start saved
				</Button>
				<Button type="button" size="sm" variant="outline" onclick={onReview}>
					<Eye data-icon="inline-start" />
					Review
				</Button>
				<Button type="button" size="sm" variant="ghost" onclick={dismiss}>
					<X data-icon="inline-start" />
					Skip
				</Button>
			</span>
		</AlertDescription>
	</Alert>
{/if}
