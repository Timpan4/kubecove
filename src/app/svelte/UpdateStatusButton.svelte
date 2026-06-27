<script lang="ts">
	import {
		AlertTriangle,
		CheckCircle2,
		Download,
		RefreshCw,
		RotateCw,
		X,
	} from "lucide-svelte";
	import { Badge, Button } from "@/components/ui/svelte";
	import { isAppUpdatesEnabled } from "@/lib/release-channel";
	import { cnfast } from "@/lib/utils";
	import type { AppUpdateState } from "@/features/app-updates/store";
	import { appUpdateStore } from "./appUpdateStore";

	const checkedAtFormatter = new Intl.DateTimeFormat(undefined, {
		dateStyle: "medium",
		timeStyle: "short",
	});
	const updatesEnabled = isAppUpdatesEnabled();
	const update = $derived($appUpdateStore);
	let manualOpen = $state(false);
	let autoOpenedVersion = $state<string | null>(null);

	const hasUpdate = $derived(update.status === "available" || update.status === "downloading");
	const autoOpenVersion = $derived(
		updatesEnabled &&
			update.status === "available" &&
			update.availableVersion !== null &&
			update.dismissedVersion !== update.availableVersion
			? update.availableVersion
			: null,
	);
	const panelOpen = $derived(
		manualOpen || (autoOpenVersion !== null && autoOpenedVersion !== autoOpenVersion),
	);
	const tooltip = $derived(updateTooltip(update));

	function formatCheckedAt(value: string | null): string {
		if (!value) return "Not checked yet";
		const date = new Date(value);
		return Number.isNaN(date.getTime()) ? "Not checked yet" : checkedAtFormatter.format(date);
	}

	function updateTooltip(state: AppUpdateState): string {
		if (state.status === "available") return "Update available";
		if (state.status === "downloading") return "Downloading update";
		if (state.status === "installed") return "Restart to finish update";
		if (state.status === "error") return "Update check failed";
		if (state.status === "checking") return "Checking for updates";
		return "Check for updates";
	}

	function closePanel() {
		manualOpen = false;
		if (autoOpenVersion !== null) autoOpenedVersion = autoOpenVersion;
	}

	function dismissAvailableUpdate() {
		if (update.availableVersion) update.dismissUpdate(update.availableVersion);
		closePanel();
	}
</script>

{#if updatesEnabled}
	<div class="relative mr-1 [-webkit-app-region:no-drag]">
		<Button
			type="button"
			variant="ghost"
			size="icon"
			class="relative size-8 text-muted-foreground"
			title={tooltip}
			aria-label={tooltip}
			aria-expanded={panelOpen}
			onclick={() => (manualOpen = !manualOpen)}
		>
			<RotateCw class={cnfast(update.status === "checking" && "animate-spin")} />
			{#if hasUpdate || update.status === "installed"}
				<span class="absolute right-1 top-1 size-2 rounded-full bg-primary"></span>
			{/if}
		</Button>

		{#if panelOpen}
			<div
				class="absolute right-0 top-[calc(100%+0.375rem)] z-popover flex w-80 max-w-[calc(100vw-1rem)] flex-col gap-3 rounded-md border bg-surface-2 p-4 text-popover-foreground shadow-xl"
				role="dialog"
				aria-label="App updates"
			>
				<div class="flex items-start justify-between gap-3">
					<div class="min-w-0">
						<div class="text-sm font-semibold text-foreground">App updates</div>
						<div class="mt-1 text-xs text-muted-foreground">Current version {update.currentVersion}</div>
					</div>
					{#if update.availableVersion}
						<Badge variant="secondary">{update.availableVersion}</Badge>
					{/if}
				</div>

				{#if update.status === "available"}
					<div class="flex flex-col gap-3">
						<div class="text-sm text-foreground">KubeCove {update.availableVersion} is available.</div>
						{#if update.releaseNotes}
							<div
								class="max-h-28 overflow-y-auto rounded-sm border bg-muted/40 p-2 text-xs leading-relaxed text-muted-foreground"
							>
								{update.releaseNotes}
							</div>
						{/if}
						<div class="flex justify-end gap-2">
							<Button type="button" variant="ghost" onclick={dismissAvailableUpdate}>
								<X data-icon="inline-start" />
								Later
							</Button>
							<Button type="button" onclick={() => void update.installUpdate()}>
								<Download data-icon="inline-start" />
								Install
							</Button>
						</div>
					</div>
				{:else if update.status === "downloading"}
					<div class="flex flex-col gap-2">
						<div class="text-sm text-foreground">Downloading update...</div>
						<div class="h-2 overflow-hidden rounded-full bg-muted">
							<div
								class="h-full bg-primary transition-all"
								style={`width: ${update.downloadProgress ?? 12}%`}
							></div>
						</div>
						<div class="text-xs text-muted-foreground">
							{update.downloadProgress === null ? "Preparing installer" : `${update.downloadProgress}%`}
						</div>
					</div>
				{:else if update.status === "installed"}
					<div class="flex flex-col gap-3">
						<div class="flex items-center gap-2 text-sm text-foreground">
							<CheckCircle2 data-icon="inline-start" />
							Update installed. Restart KubeCove to finish.
						</div>
						<Button type="button" onclick={() => void update.relaunchApp()}>
							<RefreshCw data-icon="inline-start" />
							Relaunch
						</Button>
					</div>
				{:else if update.status === "upToDate"}
					<div class="flex items-center gap-2 text-sm text-muted-foreground">
						<CheckCircle2 data-icon="inline-start" />
						KubeCove is up to date.
					</div>
				{:else if update.status === "error"}
					<div class="flex flex-col gap-2">
						<div class="flex items-center gap-2 text-sm text-destructive">
							<AlertTriangle data-icon="inline-start" />
							Update check failed.
						</div>
						{#if update.errorMessage}
							<div class="text-xs leading-relaxed text-muted-foreground">{update.errorMessage}</div>
						{/if}
					</div>
				{:else}
					<div class="text-sm text-muted-foreground">
						{update.status === "checking" ? "Checking for updates..." : "Ready to check."}
					</div>
				{/if}

				<div class="flex items-center justify-between gap-3 border-t pt-3">
					<div class="text-xs text-muted-foreground">
						Last checked: {formatCheckedAt(update.lastCheckedAt)}
					</div>
					<Button
						type="button"
						variant="outline"
						size="sm"
						disabled={update.status === "checking" || update.status === "downloading"}
						onclick={() => void update.checkForUpdates({ manual: true })}
					>
						<RefreshCw data-icon="inline-start" />
						Check
					</Button>
				</div>
			</div>
		{/if}
	</div>
{/if}
