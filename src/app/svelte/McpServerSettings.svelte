<script lang="ts">
	import { onMount } from "svelte";
	import { Badge, Button, FieldGroup, Input, Switch } from "@/components/ui/svelte";
	import { copyTextWithAnnouncement } from "@/components/copy-text";
	import { messageFromError } from "@/lib/error-redaction";
	import {
		claudeCodeCommand,
		getMcpStatus,
		type McpStatus,
		revealMcpToken,
		rotateMcpToken,
		setMcpSettings,
	} from "@/lib/mcp-runtime";
	import SettingsRow from "./SettingsRow.svelte";

	let { rows }: { rows: Record<"server" | "port" | "access", { title: string; description: string }> } = $props();

	let status = $state<McpStatus | null>(null);
	let portText = $state("");
	let pending = $state(false);
	let error = $state<string | null>(null);
	let announcement = $state("");

	const port = $derived(Number(portText));
	const portValid = $derived(/^\d+$/.test(portText.trim()) && port >= 1 && port <= 65_535);

	onMount(() => {
		void run(getMcpStatus).then(syncPort);
	});

	/** Applies an action's status. Only loading and saving the port reset the port draft. */
	async function run(action: () => Promise<McpStatus>): Promise<McpStatus | null> {
		pending = true;
		error = null;
		try {
			status = await action();
			return status;
		} catch (reason) {
			error = messageFromError(reason);
			return null;
		} finally {
			pending = false;
		}
	}

	function syncPort(saved: McpStatus | null) {
		if (saved) portText = String(saved.port);
	}

	/** Busy while copying, so a rotation cannot revoke the token being copied. */
	async function copyWithToken(label: string, format: (token: string) => string) {
		pending = true;
		announcement = "";
		try {
			const token = await revealMcpToken();
			await copyTextWithAnnouncement((message) => (announcement = message), navigator.clipboard, format(token), label);
		} catch (reason) {
			error = messageFromError(reason);
		} finally {
			pending = false;
		}
	}
</script>

<FieldGroup>
	<SettingsRow
		title={rows.server.title}
		description={status?.running && status.url
			? `Listening at ${status.url} with ${status.tools.length} read-only tools.`
			: rows.server.description}
	>
		<div class="flex items-center gap-2">
			{#if status?.enabled}
				<Badge variant={status.running ? "secondary" : "destructive"}>{status.running ? "Running" : "Stopped"}</Badge>
			{/if}
			<Switch
				checked={status?.enabled ?? false}
				disabled={pending || !status}
				onCheckedChange={(enabled) => status && void run(() => setMcpSettings(enabled, status!.port))}
				aria-label={rows.server.title}
			/>
		</div>
	</SettingsRow>

	<SettingsRow {...rows.port}>
		<form
			class="flex items-center gap-2"
			onsubmit={(event) => {
				event.preventDefault();
				if (status && portValid) void run(() => setMcpSettings(status!.enabled, port)).then(syncPort);
			}}
		>
			<Input
				id="mcp-port"
				class="w-24"
				inputmode="numeric"
				aria-label={rows.port.title}
				bind:value={portText}
				disabled={pending || !status}
			/>
			<Button type="submit" size="sm" variant="outline" disabled={pending || !status || !portValid || port === status.port}>
				Save
			</Button>
		</form>
	</SettingsRow>

	<SettingsRow {...rows.access}>
		<div class="flex flex-wrap items-center gap-2">
			<Button
				type="button"
				size="sm"
				variant="outline"
				disabled={pending || !status?.url}
				onclick={() => {
					// Captured before the token read, which can outlast a listener restart.
					const url = status?.url;
					if (url) void copyWithToken("Claude Code command", (token) => claudeCodeCommand(url, token));
				}}
			>
				Copy Claude Code command
			</Button>
			<Button type="button" size="sm" variant="outline" disabled={pending || !status} onclick={() => void copyWithToken("token", (token) => token)}>
				Copy token
			</Button>
			<Button type="button" size="sm" variant="outline" disabled={pending || !status} onclick={() => void run(rotateMcpToken)}>
				Rotate token
			</Button>
		</div>
	</SettingsRow>
</FieldGroup>
{#if status?.error}<p role="alert" class="mt-3 text-sm text-destructive">{status.error}</p>{/if}
{#if error}<p role="alert" class="mt-3 text-sm text-destructive">{error}</p>{/if}
<p class="mt-3 text-sm text-muted-foreground" aria-live="polite">{announcement}</p>
