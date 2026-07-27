<script lang="ts">
	import {
		Checkbox,
		Field,
		FieldDescription,
		FieldGroup,
		FieldLabel,
		Label,
		Select,
		SelectContent,
		SelectGroup,
		SelectItem,
		SelectTrigger,
		SelectValue,
		Textarea,
	} from "@/components/ui/svelte";
	import { podExecTarget, type PodExecPreset } from "@/features/live-sessions";
	import type { ResourceSummary } from "@/lib/types";
	import type { ContainerStatusRow } from "./helpers";

	let {
		resource,
		containers,
		selectedContainer = $bindable(""),
		preset = $bindable("sh"),
		customArgv = $bindable(""),
		confirmed = $bindable(false),
		showKubeconfigSourceLabels,
		kubeconfigSourceKey,
		commandText,
	}: {
		resource: ResourceSummary;
		containers: ContainerStatusRow[];
		selectedContainer: string;
		preset: PodExecPreset;
		customArgv: string;
		confirmed: boolean;
		showKubeconfigSourceLabels: boolean;
		kubeconfigSourceKey?: string;
		commandText: string;
	} = $props();

	const containerOptions = $derived(
		containers.filter((container) => container.type !== "init").length > 0
			? containers.filter((container) => container.type !== "init")
			: containers,
	);
</script>

<FieldGroup>
	<Field>
		<FieldLabel>Container</FieldLabel>
		<Select
			value={selectedContainer || "__default"}
			items={[
				{ value: "__default", label: "Default container" },
				...containerOptions.map((container) => ({
					value: container.name,
					label: container.name,
				})),
			]}
			onValueChange={(value: string) => {
				selectedContainer = value === "__default" ? "" : value;
				confirmed = false;
			}}
		>
			<SelectTrigger class="w-full">
				<SelectValue placeholder="Default container" />
			</SelectTrigger>
			<SelectContent>
				<SelectGroup>
					<SelectItem value="__default">Default container</SelectItem>
					{#each containerOptions as container (`${container.type}:${container.name}`)}
						<SelectItem value={container.name}>{container.name}</SelectItem>
					{/each}
				</SelectGroup>
			</SelectContent>
		</Select>
		<FieldDescription>Kubernetes chooses default only when no container is selected.</FieldDescription>
	</Field>
	<Field>
		<FieldLabel>Command</FieldLabel>
		<Select
			value={preset}
			items={[
				{ value: "sh", label: "/bin/sh" },
				{ value: "bash", label: "/bin/bash" },
				{ value: "custom", label: "Custom argv" },
			]}
			onValueChange={(value: string) => {
				preset = value as PodExecPreset;
				confirmed = false;
			}}
		>
			<SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
			<SelectContent>
				<SelectGroup>
					<SelectItem value="sh">/bin/sh</SelectItem>
					<SelectItem value="bash">/bin/bash</SelectItem>
					<SelectItem value="custom">Custom argv</SelectItem>
				</SelectGroup>
			</SelectContent>
		</Select>
		<FieldDescription>Presets are exact commands; no local shell parsing.</FieldDescription>
	</Field>
	{#if preset === "custom"}
		<Field>
			<FieldLabel>Custom argv</FieldLabel>
			<Textarea
				bind:value={customArgv}
				placeholder={"/usr/bin/env\nprintenv"}
				oninput={() => (confirmed = false)}
			/>
			<FieldDescription>One argv item per line.</FieldDescription>
		</Field>
	{/if}
</FieldGroup>

<div class="rounded-md border bg-muted/20 p-3 text-xs">
	<div class="font-medium">Target</div>
	<div class="mt-1 break-words font-mono text-muted-foreground">
		{podExecTarget(resource, selectedContainer)}
	</div>
	{#if showKubeconfigSourceLabels && kubeconfigSourceKey}
		<div class="mt-2 font-medium">Kubeconfig source</div>
		<div class="mt-1 break-words font-mono text-muted-foreground">{kubeconfigSourceKey}</div>
	{/if}
	<div class="mt-3 font-medium">Command</div>
	<div class="mt-1 break-words font-mono text-muted-foreground">
		{commandText || "Custom argv is required"}
	</div>
</div>

<Label class="gap-2 rounded-md border bg-background p-3 text-xs text-muted-foreground">
	<Checkbox checked={confirmed} onCheckedChange={(checked) => (confirmed = checked)} />
	I understand this opens an interactive session in the selected Pod.
</Label>
