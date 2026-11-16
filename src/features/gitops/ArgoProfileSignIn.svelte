<script lang="ts">
	import { Button, Input, SegmentedControl } from "@/components/ui/svelte";

	// A saved profile that does not remember its credential asks for one in its own row,
	// so a credential typed in the new-connection form never reaches this server.
	let {
		label,
		local = $bindable(false),
		token = $bindable(""),
		username = $bindable(""),
		password = $bindable(""),
		busy,
		onSubmit,
		onCancel,
	}: {
		label: string;
		local?: boolean;
		token?: string;
		username?: string;
		password?: string;
		busy: boolean;
		onSubmit: () => void;
		onCancel: () => void;
	} = $props();

	const ready = $derived(local ? Boolean(username && password) : Boolean(token));
</script>

<div class="flex w-full flex-col gap-2 border-t pt-2" role="group" aria-label={`Sign in to ${label}`}>
	<p class="text-xs text-muted-foreground">This profile does not remember its credential. Enter one for this server.</p>
	<SegmentedControl
		value={local ? "local" : "token"}
		options={[
			{ value: "token", label: "Token" },
			{ value: "local", label: "Username and password" },
		]}
		onChange={(value: "token" | "local") => (local = value === "local")}
		ariaLabel={`Sign-in method for ${label}`}
	/>
	{#if local}
		<Input aria-label={`Username for ${label}`} bind:value={username} autocomplete="username" />
		<Input aria-label={`Password for ${label}`} bind:value={password} type="password" autocomplete="current-password" />
	{:else}
		<Input aria-label={`Token for ${label}`} bind:value={token} type="password" autocomplete="off" />
	{/if}
	<div class="flex gap-2">
		<Button size="sm" type="button" disabled={busy || !ready} onclick={onSubmit}>Sign in</Button>
		<Button size="sm" variant="ghost" type="button" aria-label="Cancel sign-in" onclick={onCancel}>Cancel</Button>
	</div>
</div>
