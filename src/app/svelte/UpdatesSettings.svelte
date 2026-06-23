<script lang="ts">
	import { Download, RefreshCw } from "lucide-svelte";
	import { Badge, Button, FieldGroup } from "@/components/ui/svelte";
	import { isAppUpdatesEnabled } from "@/lib/release-channel";
	import SettingsRow from "./SettingsRow.svelte";
	import { appUpdateActions, appUpdateStore } from "./appUpdateStore";

	const checkedAtFormatter = new Intl.DateTimeFormat(undefined, {
		dateStyle: "medium",
		timeStyle: "short",
	});
	const updatesEnabled = isAppUpdatesEnabled();
	const update = $derived($appUpdateStore);

	const updateBusy = $derived(
		update.status === "checking" || update.status === "downloading",
	);

	function formatCheckedAt(value: string | null): string {
		if (!value) return "Not checked yet";
		const date = new Date(value);
		return Number.isNaN(date.getTime())
			? "Not checked yet"
			: checkedAtFormatter.format(date);
	}
</script>

<FieldGroup>
	<SettingsRow
		title="KubeCove version"
		description={updatesEnabled
			? `Current version ${update.currentVersion}. Last checked: ${formatCheckedAt(update.lastCheckedAt)}.`
			: `Current version ${update.currentVersion}. Development build; update checks disabled.`}
	>
		<div class="flex items-center gap-2">
			{#if updatesEnabled && update.availableVersion}
				<Badge variant="secondary">Update {update.availableVersion}</Badge>
			{/if}
			{#if updatesEnabled}
				<Button
					type="button"
					variant="outline"
					size="sm"
					disabled={updateBusy}
					onclick={() => void appUpdateActions.checkForUpdates({ manual: true })}
				>
					<RefreshCw data-icon="inline-start" />
					Check
				</Button>
			{:else}
				<Badge variant="outline">Dev build</Badge>
			{/if}
		</div>
	</SettingsRow>

	{#if updatesEnabled && update.status === "available"}
		<SettingsRow
			title="Update available"
			description={`KubeCove ${update.availableVersion} can be downloaded and installed now.`}
		>
			<Button
				type="button"
				size="sm"
				onclick={() => void appUpdateActions.installUpdate()}
			>
				<Download data-icon="inline-start" />
				Install
			</Button>
		</SettingsRow>
	{/if}

	{#if updatesEnabled && update.status === "downloading"}
		<SettingsRow
			title="Installing update"
			description={update.downloadProgress === null
				? "Preparing installer."
				: `Download progress: ${update.downloadProgress}%.`}
		>
			<Badge variant="secondary">Working</Badge>
		</SettingsRow>
	{/if}

	{#if updatesEnabled && update.status === "installed"}
		<SettingsRow
			title="Relaunch required"
			description="Update is installed. Relaunch KubeCove to finish."
		>
			<Button
				type="button"
				size="sm"
				onclick={() => void appUpdateActions.relaunchApp()}
			>
				<RefreshCw data-icon="inline-start" />
				Relaunch
			</Button>
		</SettingsRow>
	{/if}

	{#if updatesEnabled && update.status === "error" && update.errorMessage}
		<SettingsRow title="Update check failed" description={update.errorMessage}>
			<Button
				type="button"
				variant="outline"
				size="sm"
				onclick={() => void appUpdateActions.checkForUpdates({ manual: true })}
			>
				<RefreshCw data-icon="inline-start" />
				Retry
			</Button>
		</SettingsRow>
	{/if}
</FieldGroup>
