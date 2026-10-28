<script lang="ts">
	import { useQueryClient } from "@tanstack/svelte-query";
	import { eligibleArgoProfiles } from "@/lib/argo-connection-policy";
	import { queryMatchers } from "@/lib/queryKeys";
	import { settingsStore } from "@/lib/settings-store";
	import { createTauriClient } from "@/lib/tauri";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import { reconnectRememberedArgoProfiles } from "./argo-connection";

	let { context }: { context: WorkspaceReadContext } = $props();
	const client = createTauriClient();
	const queryClient = useQueryClient();
	// One silent attempt per profile while the workspace is open; a failed attempt waits for
	// a manual reconnect in the Argo CD connection dialog.
	const attempted = new Set<string>();

	$effect(() => {
		if (!context.sourceReady) return;
		const profiles = eligibleArgoProfiles(
			$settingsStore.argoProfiles,
			context.clusterContext,
			context.workspaceId,
			context.kubeconfigSourceKey ?? "",
		).filter((profile) => profile.rememberCredential && !attempted.has(profile.id));
		if (profiles.length === 0) return;
		for (const profile of profiles) attempted.add(profile.id);
		const scope = {
			clusterContext: context.clusterContext,
			workspaceId: context.workspaceId,
			kubeconfigSourceKey: context.kubeconfigSourceKey,
		};
		void reconnectRememberedArgoProfiles(client, scope, profiles).then(() =>
			queryClient.invalidateQueries({ predicate: queryMatchers.argoConnectionStatuses() }),
		);
	});
</script>
