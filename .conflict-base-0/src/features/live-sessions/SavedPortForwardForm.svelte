<script lang="ts">
	import { Save, X } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import { Button, Input } from "@/components/ui/svelte";
	import type { SavedPortForwardFormValues } from "./portForwardForms";

	let {
		form,
		error,
		editingId,
		onValue,
		onCancel,
		onSubmit,
	}: {
		form: SavedPortForwardFormValues;
		error: string | null;
		editingId: string | null;
		onValue: (key: keyof SavedPortForwardFormValues, value: string) => void;
		onCancel: () => void;
		onSubmit: () => void;
	} = $props();

	function inputValue(event: Event): string {
		return (event.currentTarget as HTMLInputElement).value;
	}
</script>

<div class="rounded-md border bg-muted/20 p-3">
	<div class="grid gap-3 md:grid-cols-3">
		<Input
			value={form.label}
			placeholder="Label"
			aria-label="Saved forward label"
			oninput={(event: Event) => onValue("label", inputValue(event))}
		/>
		<Input
			value={form.clusterContext}
			placeholder="Cluster context"
			aria-label="Saved forward cluster context"
			oninput={(event: Event) => onValue("clusterContext", inputValue(event))}
		/>
		<Input
			value={form.namespace}
			placeholder="Namespace"
			aria-label="Saved forward namespace"
			oninput={(event: Event) => onValue("namespace", inputValue(event))}
		/>
		<Input
			value={form.serviceName}
			placeholder="Service name"
			aria-label="Saved forward Service name"
			oninput={(event: Event) => onValue("serviceName", inputValue(event))}
		/>
		<Input
			value={form.servicePort}
			placeholder="Service port"
			aria-label="Saved forward Service port"
			inputmode="numeric"
			oninput={(event: Event) => onValue("servicePort", inputValue(event))}
		/>
		<Input
			value={form.localPort}
			placeholder="Local port (auto)"
			aria-label="Saved forward local port"
			inputmode="numeric"
			oninput={(event: Event) => onValue("localPort", inputValue(event))}
		/>
	</div>
	{#if error}
		<FriendlyError
			class="mt-3"
			{error}
			context={{ operation: "portForward", fallbackTitle: "Check saved forward" }}
		/>
	{/if}
	<div class="mt-3 flex justify-end gap-2">
		<Button type="button" variant="outline" size="sm" onclick={onCancel}>
			<X data-icon="inline-start" />
			Cancel
		</Button>
		<Button type="button" size="sm" onclick={onSubmit}>
			<Save data-icon="inline-start" />
			{editingId ? "Save changes" : "Save forward"}
		</Button>
	</div>
</div>
