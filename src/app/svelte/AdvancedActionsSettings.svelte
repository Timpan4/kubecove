<script lang="ts">
	import { AlertTriangle } from "lucide-svelte";
	import { Button, FieldGroup, Switch } from "@/components/ui/svelte";
	import { settingsStore } from "@/lib/settings-store";
	import SettingsRow from "./SettingsRow.svelte";

	const settings = $derived($settingsStore);
	let confirmingEnable = $state(false);

	function changeSkipReview(skip: boolean): void {
		if (skip) {
			confirmingEnable = true;
			return;
		}
		confirmingEnable = false;
		settings.setSkipChangeReview(false);
	}

	function confirmEnable(): void {
		settings.setSkipChangeReview(true);
		confirmingEnable = false;
	}
</script>

<FieldGroup>
	<SettingsRow
		title="Skip change review"
		description="Successful dry-runs apply without showing a diff, across every context including production. Dry-run remains required, and deletes always ask. Turning this off takes effect immediately."
	>
		<Switch
			checked={settings.skipChangeReview}
			onCheckedChange={changeSkipReview}
			aria-label="Skip change review"
		/>
	</SettingsRow>
	{#if confirmingEnable}
		<div
			role="alert"
			class="flex gap-3 rounded-md border border-warning/40 bg-warning-surface p-3 text-warning-foreground"
		>
			<AlertTriangle class="mt-0.5 size-4 shrink-0" />
			<div class="min-w-0 flex-1 space-y-2">
				<div>
					<p class="text-sm font-medium">Changes can apply without review</p>
					<p class="text-sm">
						A successful dry-run will apply changes without showing the diff, even in production.
						A mistaken value may go live immediately. Dry-run still runs, and deletes always
						require confirmation.
					</p>
				</div>
				<div class="flex flex-wrap gap-2">
					<Button
						type="button"
						variant="outline"
						size="sm"
						onclick={() => (confirmingEnable = false)}
					>
						Keep review
					</Button>
					<Button type="button" variant="destructive" size="sm" onclick={confirmEnable}>
						Skip review
					</Button>
				</div>
			</div>
		</div>
	{/if}
</FieldGroup>
