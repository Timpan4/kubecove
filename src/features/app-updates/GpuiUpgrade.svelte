<script lang="ts">
	import { onMount } from "svelte";
	import { Button } from "@/components/ui/svelte";
	import {
		checkGpuiUpgrade,
		type GpuiUpgradeInfo,
		installGpuiUpgrade,
		isAppError,
		launchGpuiUpgrade,
		openDesktopReleases,
		revealGpuiUpgrade,
	} from "@/lib/tauri";

	let { updatesEnabled }: { updatesEnabled: boolean } = $props();
	let info = $state<GpuiUpgradeInfo | null>(null);
	let working = $state("");
	let error = $state("");

	async function run(message: string, task: () => Promise<void>) {
		if (working) return;
		working = message;
		error = "";
		try {
			await task();
		} catch (cause) {
			error = cause instanceof Error || isAppError(cause) ? cause.message : String(cause);
		} finally {
			working = "";
		}
	}

	function check() {
		return run("Checking for a GPUI release…", async () => {
			info = await checkGpuiUpgrade();
		});
	}

	function install(version: string) {
		return run("Preparing and verifying the GPUI download…", async () => {
			const result = await installGpuiUpgrade(version);
			if (result) info = result;
		});
	}

	onMount(() => {
		if (updatesEnabled) void check();
	});
</script>

<section aria-labelledby="gpui-upgrade-title" class="space-y-3 rounded-md border bg-card p-3 text-sm">
	<h3 id="gpui-upgrade-title" class="font-semibold">Switch to KubeCove GPUI</h3>
	<p>Check for the native desktop version of KubeCove.</p>
	<ul class="list-disc space-y-1 pl-5 text-muted-foreground">
		<li>GPUI uses your existing kubeconfig sources, MCP settings and token, and saved remote server credentials.</li>
		<li>KubeCove saves your workspaces, preferences, and Argo CD profile metadata for migration. For the side-by-side GPUI app, use workspace JSON export and import.</li>
		<li>If MCP is enabled, open the installation folder and quit Tauri before starting GPUI so the MCP port is free.</li>
	</ul>
	<div aria-live="polite">
		{#if working}
			<p>{working}</p>
		{:else if info?.status === "unsupported"}
			<p>{info.message}</p>
		{:else if info?.status === "consolidated"}
			<p>KubeCove {info.version} is available as a separate download.</p>
		{:else if info?.status === "unavailable"}
			<p>No compatible GPUI release is available for {info.target}. Check GitHub Releases for KubeCove 0.16 or later.</p>
		{:else if info?.status === "available"}
			<p>KubeCove GPUI {info.version} is available for {info.target}. A compatible consolidated release has not been published yet.</p>
		{:else if info?.status === "installed"}
			<p class="break-all">{info.installKind === "windowsInstaller" ? "Verified installer" : "Installed GPUI"} {info.version} at {info.path}</p>
			{#if !info.canLaunch}
				<p>KubeCove could not verify that only you or an administrator can change this installation. Open its folder to inspect it.</p>
			{/if}
		{/if}
	</div>
	{#if error}<p role="alert" class="text-destructive">{error}</p>{/if}
	{#if info?.status === "available" && info.notes}
		<details><summary>GPUI release notes</summary><p class="mt-2 whitespace-pre-wrap">{info.notes}</p></details>
	{/if}
	<div class="flex flex-wrap gap-2">
		{#if info?.status === "consolidated" || info?.status === "unavailable"}
			<Button size="sm" disabled={!!working} onclick={() => void run("Opening GitHub Releases…", openDesktopReleases)}>Open GitHub Releases</Button>
		{/if}
		{#if info?.status === "available"}
			{@const version = info.version}
			<Button size="sm" disabled={!!working} onclick={() => void install(version)}>
				{info.installKind === "windowsInstaller" ? "Download GPUI installer" : "Choose GPUI install folder"}
			</Button>
		{:else if info?.status === "installed"}
			{#if info.canLaunch}
				<Button size="sm" disabled={!!working} onclick={() => void run("Opening GPUI…", launchGpuiUpgrade)}>
					{info.installKind === "windowsInstaller" ? "Run GPUI installer" : "Launch GPUI"}
				</Button>
			{/if}
			<Button size="sm" variant="outline" disabled={!!working} onclick={() => void run("Opening installation folder…", revealGpuiUpgrade)}>
				Open installation folder
			</Button>
		{/if}
		{#if !working && !info && !error && !updatesEnabled}
			<Button size="sm" variant="outline" onclick={() => void check()}>Check for GPUI release</Button>
		{:else if !working && (error || info?.status === "unavailable") && info?.status !== "installed"}
			<Button size="sm" variant="outline" onclick={() => void check()}>Check GPUI release again</Button>
		{/if}
	</div>
</section>
