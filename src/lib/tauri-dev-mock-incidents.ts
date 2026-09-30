import type { JsonObject } from "./types";

export const evictedWorkerName = "operations-worker-evicted";

// Screenshot fixture. The current termination time was not reported.
const storagePressure = "The node was low on resource: ephemeral-storage. Threshold quantity: 75100960885, available: 49664872Ki.";
export const evictedWorkerStatus: JsonObject = {
	phase: "Failed",
	reason: "Evicted",
	message: storagePressure,
	conditions: [
		{
			type: "ContainersReady",
			status: "False",
			reason: "PodFailed",
			lastTransitionTime: "2026-09-30T11:52:38+02:00",
		},
		{
			type: "Ready",
			status: "False",
			reason: "PodFailed",
			lastTransitionTime: "2026-09-30T11:52:38+02:00",
		},
		{
			type: "DisruptionTarget",
			status: "True",
			reason: "TerminationByKubelet",
			message: storagePressure,
			lastTransitionTime: "2026-09-30T11:56:56+02:00",
		},
		{
			type: "PodReadyToStartContainers",
			status: "False",
			lastTransitionTime: "2026-09-30T11:56:56+02:00",
		},
	],
	containerStatuses: [
		{
			name: "worker",
			ready: false,
			restartCount: 3,
			state: {
				terminated: {
					reason: "ContainerStatusUnknown",
					message: "The container could not be located when the pod was terminated",
					exitCode: 137,
				},
			},
			lastState: {
				terminated: {
					reason: "Error",
					exitCode: 1,
					finishedAt: "2026-09-30T11:56:47+02:00",
				},
			},
		},
	],
};
