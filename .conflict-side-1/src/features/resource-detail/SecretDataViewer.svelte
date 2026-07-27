<script lang="ts">
	import { Button, Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/svelte";
	import {
		decodeSecretDataValue,
		maskedSecretValue,
		parseSecretData,
		type SecretDataEntry,
	} from "./secretData";
	import { revealSecretDataValue, type TauriClient } from "@/lib/tauri";

	let { client, clusterContext, name, namespace, kubeconfigSourceKey, yamlText, contextKey, active }: { client: TauriClient; clusterContext: string; name: string; namespace: string | null; kubeconfigSourceKey?: string; yamlText: string; contextKey: string; active: boolean } = $props();
	let entries = $derived(parseSecretData(yamlText));
	let revealedValues = $state<Record<string, string>>({});
	let revealErrors = $state<Record<string, string>>({});
	let revealEpoch = 0;
	const revealTimers = new Map<string, ReturnType<typeof setTimeout>>();

	function clearReveals() {
		for (const timer of revealTimers.values()) clearTimeout(timer);
		revealTimers.clear();
		revealedValues = {};
		revealErrors = {};
		revealEpoch += 1;
	}

	async function reveal(entry: SecretDataEntry) {
		if (entry.source !== "data" || !namespace) return;
		const epoch = revealEpoch;
		let encodedValue: string;
		try {
			encodedValue = await revealSecretDataValue(client, clusterContext, name, namespace, entry.key, kubeconfigSourceKey);
		} catch (error) {
			if (epoch === revealEpoch) revealErrors = { ...revealErrors, [entry.id]: error instanceof Error ? error.message : "Unable to reveal value" };
			return;
		}
		if (epoch !== revealEpoch) return;
		const result = decodeSecretDataValue(encodedValue);
		if (result.value === undefined) {
			revealErrors = { ...revealErrors, [entry.id]: "Binary value cannot be shown as UTF-8" };
			return;
		}
		revealedValues = { ...revealedValues, [entry.id]: result.value };
		const existingTimer = revealTimers.get(entry.id);
		if (existingTimer) clearTimeout(existingTimer);
		revealTimers.set(
			entry.id,
			setTimeout(() => {
				const { [entry.id]: _discarded, ...remaining } = revealedValues;
				revealedValues = remaining;
				revealTimers.delete(entry.id);
			}, 60_000),
		);
	}

	function hide(entryId: string) {
		const timer = revealTimers.get(entryId);
		if (timer) clearTimeout(timer);
		revealTimers.delete(entryId);
		const { [entryId]: _discarded, ...remaining } = revealedValues;
		revealedValues = remaining;
	}

	$effect(() => {
		yamlText;
		contextKey;
		active;
		clearReveals();
		return clearReveals;
	});
</script>

{#if active && entries.length > 0}
		<Card class="mb-3">
			<CardHeader>
				<CardTitle>Secret data</CardTitle>
				<CardDescription>Values stay masked until revealed. Revealed values clear after 60 seconds.</CardDescription>
			</CardHeader>
			<CardContent>
				<dl class="flex flex-col gap-3">
					{#each entries as entry (entry.id)}
						<div class="flex flex-col gap-2 rounded-md border p-3">
							<div class="flex flex-wrap items-center justify-between gap-2">
								<dt class="font-medium">{entry.key} <span class="font-normal text-muted-foreground">({entry.source})</span></dt>
								{#if revealedValues[entry.id] !== undefined}
									<Button type="button" variant="outline" size="sm" onclick={() => hide(entry.id)}>Hide</Button>
								{:else if entry.source !== "data" || !namespace}
									<span class="text-muted-foreground">Unavailable</span>
								{:else}
									<Button type="button" variant="outline" size="sm" onclick={() => reveal(entry)}>Reveal</Button>
								{/if}
							</div>
							<dd class="min-w-0 break-all rounded bg-muted px-2 py-1 font-mono text-xs">{maskedSecretValue()}</dd>
							{#if revealedValues[entry.id] !== undefined}
								<dd class="min-w-0 whitespace-pre-wrap break-words rounded bg-muted px-2 py-1 font-mono text-xs">{revealedValues[entry.id]}</dd>
							{:else if revealErrors[entry.id]}
								<dd class="text-muted-foreground">{revealErrors[entry.id]}</dd>
							{/if}
						</div>
					{/each}
				</dl>
			</CardContent>
		</Card>
{/if}
