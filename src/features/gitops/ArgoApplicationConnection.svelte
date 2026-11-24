<script lang="ts">
	import { Button, Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/svelte";
	import type { argoApplicationInspection } from "@/lib/argo-application-inspection.svelte";
	import { argoConnectionPreferenceValue, normalizeArgoConnectionPreference } from "@/lib/argo-connection-policy";
	import { settingsStore } from "@/lib/settings-store";
	import type { ArgoServerEndpoint } from "@/lib/types";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import ArgoConnectionDialog from "./ArgoConnectionDialog.svelte";

	let { inspection, workspaceReadContext }: {
		inspection: ReturnType<typeof argoApplicationInspection>;
		workspaceReadContext: WorkspaceReadContext;
	} = $props();
	const workspaceId = $derived(workspaceReadContext.workspaceId);
	const clusterContext = $derived(workspaceReadContext.clusterContext);
	const kubeconfigEnvVar = $derived(workspaceReadContext.kubeconfigSourceKey);
	const connectionPolicy = $derived(inspection.current.choice);
	const transport = $derived(connectionPolicy.transport);
	let argoConnectionOpen = $state(false);
	const preferenceValue = $derived(
		argoConnectionPreferenceValue(connectionPolicy.preference),
	);
	const selectedStatus = $derived(inspection.current.selectedStatus);
	const selectedProfile = $derived(connectionPolicy.selectedProfile);
	function argoEndpointLabel(endpoint: ArgoServerEndpoint): string {
		return endpoint.kind === "externalHttps"
			? endpoint.url
			: `${endpoint.namespace}/${endpoint.serviceName}:${endpoint.servicePort}`;
	}

	function setConnectionPreference(value: string) {
		$settingsStore.setArgoConnectionPreference(
			workspaceId,
			normalizeArgoConnectionPreference(value),
		);
	}

</script>

<div class="flex min-w-0 flex-wrap items-center gap-2 rounded-lg border bg-surface-1 p-3">
	<Select value={preferenceValue} onValueChange={setConnectionPreference}>
		<SelectTrigger class="w-72 max-w-full" ariaLabel="Argo CD inspection preference">
			<SelectValue>
				{connectionPolicy.preference.kind === "automatic"
					? inspection.current.checkingProfiles
						? "Automatic · Checking profiles"
						: `Automatic · ${selectedProfile ? argoEndpointLabel(selectedProfile.endpoint) : "Kubernetes"}`
					: connectionPolicy.preference.kind === "kubernetes"
						? "Kubernetes"
						: selectedProfile
								? argoEndpointLabel(selectedProfile.endpoint)
								: connectionPolicy.preference.profileId}
			</SelectValue>
		</SelectTrigger>
		<SelectContent>
			<SelectGroup>
				<SelectItem value="automatic" label="Automatic">Automatic</SelectItem>
				<SelectItem value="kubernetes" label="Kubernetes">Kubernetes</SelectItem>
				{#each connectionPolicy.eligibleProfiles as profile}
					<SelectItem value={`connected:${profile.id}`} label={argoEndpointLabel(profile.endpoint)}>{argoEndpointLabel(profile.endpoint)}</SelectItem>
				{/each}
			</SelectGroup>
		</SelectContent>
	</Select>
	<div class="min-w-0 flex-1 text-xs text-muted-foreground">
		{connectionPolicy.unavailable
			? selectedStatus?.unavailableReason ?? "Reconnect this exact workspace profile."
			: transport === "kubernetes"
				? "Argo RBAC is not evaluated. Exact manifests require a connected profile."
				: "Connected Argo CD authorization is evaluated during operation preflight."}
	</div>
	<Button type="button" size="sm" variant="outline" onclick={() => (argoConnectionOpen = true)}>
		{connectionPolicy.eligibleProfiles.length > 0 ? "Manage connection" : "Connect Argo CD"}
	</Button>
</div>
<ArgoConnectionDialog
	bind:open={argoConnectionOpen}
	scope={{ clusterContext, workspaceId, kubeconfigSourceKey: kubeconfigEnvVar }}
/>
