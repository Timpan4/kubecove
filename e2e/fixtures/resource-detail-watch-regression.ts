import { QueryClient, QueryObserver } from "@tanstack/svelte-query";
import { mount, unmount } from "svelte";
import { buildResourceDetailReadSpec } from "@/features/resource-detail/resourceDetailReadSpec";
import { resources } from "@/lib/tauri-dev-mock-data";
import { createDevMockTauriClient } from "@/lib/tauri-dev-mocks";
import { createMockTauriClient } from "@/lib/tauri-runtime";
import { getSettingsSnapshot } from "@/lib/settings-store";
import { queryKeys } from "@/lib/queryKeys";
import type { ResourceDetailsFull, ResourceEventSummary, StreamMessage } from "@/lib/types";
import ResourceDetailWatchHarness from "./ResourceDetailWatchHarness.svelte";

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((done) => { resolve = done; });
	return { promise, resolve };
}

// Failure modes: a watch joins a pre-change read and clears its invalidation,
// or an exact events filter misses the component's UID-suffixed query key.
// A disabled Argo inspector can keep fetching after a tab switch. Counting it
// as an active read requeues details every 250 ms although invalidation never
// waits for that request. One watch must cause only one details refresh.
export async function resourceDetailWatchRegression(disabledArgo = false) {
	const pod = resources.find((row) => row.kind === "Pod");
	if (!pod) throw new Error("Pod fixture is missing");
	const resource = disabledArgo ? { ...pod, kind: "Application", group: "argoproj.io" } : pod;
	const base = createDevMockTauriClient();
	const details = await base.invoke<ResourceDetailsFull>("get_resource_details", {
		kind: resource.kind, name: resource.name, namespace: resource.namespace,
	});
	const events = await base.invoke<ResourceEventSummary[]>("list_resource_events", {
		kind: resource.kind, name: resource.name, namespace: resource.namespace,
	});
	if (events.length === 0) throw new Error("Event fixture is missing");
	const oldDetails = deferred<ResourceDetailsFull>();
	const oldEvents = deferred<ResourceEventSummary[]>();
	const detailsStarted = deferred<void>();
	const eventsStarted = deferred<void>();
	const resourceWatch = deferred<{ onmessage: (event: StreamMessage) => void }>();
	const eventsWatch = deferred<{ onmessage: (event: StreamMessage) => void }>();
	let detailReads = 0;
	let eventReads = 0;
	const client = createMockTauriClient({
		get_resource_details: () => {
			detailReads++;
			detailsStarted.resolve();
			return detailReads === 1 ? oldDetails.promise : {
				...details, metadata: { ...details.metadata, watchRevision: "new" },
			};
		},
		list_resource_events: () => {
			eventReads++;
			eventsStarted.resolve();
			return eventReads === 1 ? oldEvents.promise : [];
		},
		start_resource_watch: ({ channel }: { channel: { onmessage: (event: StreamMessage) => void } }) => {
			resourceWatch.resolve(channel);
			return "detail-watch";
		},
		start_resource_event_watch: ({ channel }: { channel: { onmessage: (event: StreamMessage) => void } }) => {
			eventsWatch.resolve(channel);
			return "events-watch";
		},
		stop_stream: true,
		cancel_backend_requests: { cancelled: 0 },
	});
	const queryClient = new QueryClient();
	const inspectorRead = deferred<null>();
	const inspectorOptions = {
		queryKey: queryKeys.argoWorkspaceInspector(
			resource.cluster, "watch-regression", resource.name, resource.namespace,
			String(details.metadata.uid ?? ""),
			true, "kubernetes",
		),
		queryFn: () => inspectorRead.promise,
		enabled: true,
	};
	const inspector = new QueryObserver(queryClient, inspectorOptions);
	const stopInspector = disabledArgo ? inspector.subscribe(() => {}) : () => {};
	// Model a mounted inspector becoming disabled while its request is pending.
	inspector.setOptions({ ...inspectorOptions, enabled: false });
	const settings = getSettingsSnapshot();
	const spec = buildResourceDetailReadSpec(resource, undefined, settings.yamlViewModeDefault, settings.yamlEncodingDefault);
	const freshDetails = deferred<void>();
	const freshEvents = deferred<void>();
	const stop = queryClient.getQueryCache().subscribe(({ query }) => {
		if (query.queryKey[0] === "resource-details" &&
			queryClient.getQueryData<ResourceDetailsFull>(spec.detailsQueryKey)?.metadata.watchRevision === "new") {
			freshDetails.resolve();
		}
		if (query.queryKey[0] === "resource-events" && eventReads > 1 && query.state.fetchStatus === "idle") {
			freshEvents.resolve();
		}
	});
	const target = document.createElement("div");
	document.body.append(target);
	const component = mount(ResourceDetailWatchHarness, { target, props: {
		queryClient, client, resource,
		workspaceReadContext: {
			workspaceId: "watch-regression", clusterContext: resource.cluster,
			namespaceScope: [], sourceReady: true, sourceError: null,
			showKubeconfigSourceLabels: false,
		},
	} });
	try {
		await detailsStarted.promise;
		(await resourceWatch.promise).onmessage({ type: "resourceChanged", streamId: "detail-watch", action: "modified", target: {
			cluster: resource.cluster, kind: resource.kind, name: resource.name, namespace: resource.namespace,
		} });
		oldDetails.resolve(details);
		await freshDetails.promise;
		await eventsStarted.promise;
		(await eventsWatch.promise).onmessage({ type: "resourceEventsChanged", streamId: "events-watch", action: "modified", target: {
			cluster: resource.cluster, kind: resource.kind, name: resource.name, namespace: resource.namespace,
		} });
		oldEvents.resolve(events);
		await freshEvents.promise;
		const eventsKey = [...spec.eventsQueryKey, details.metadata.uid ?? ""];
		if (queryClient.getQueryData<ResourceEventSummary[]>(eventsKey)?.length !== 0) {
			throw new Error("Events retained the pre-change snapshot");
		}
		if (disabledArgo) {
			// Observe two throttle intervals so a queued repeat has time to run.
			await new Promise((resolve) => setTimeout(resolve, 2 * 250));
			if (queryClient.isFetching({ queryKey: inspectorOptions.queryKey }) !== 1 ||
				queryClient.isFetching({ queryKey: inspectorOptions.queryKey, type: "active" }) !== 0) {
				throw new Error("Inspector must remain disabled and fetching throughout the check");
			}
			if (detailReads !== 2) throw new Error(`Disabled inspector caused repeated details reads: ${detailReads}`);
		}
		return { detailReads, eventReads, details: "new", events: [] };
	} finally {
		await unmount(component);
		stopInspector();
		inspectorRead.resolve(null);
		stop();
		queryClient.clear();
		target.remove();
	}
}
