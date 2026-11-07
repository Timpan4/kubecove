import type { JsonObject, ResourceSummary } from "./types";

// The Details tab fixture is one Pod in the `platform` namespace. Its Helm
// release, `metrics-gateway`, is the mock release in that namespace, so the
// Open Helm release action resolves to a real Helm details card.
const DETAILS_POD = "metrics-gateway-9f6d4f7c9-7j2p4";
const OWNER = "metrics-gateway-9f6d4f7c9";
// A multi-container Pod with long container names, for the Logs toolbar's container picker.
const DEX_POD = "argocd-dex-server-6d7f9c5b8-q4w2n";
const DEX_CONTAINERS = ["dex-server", "oidc-bridge-proxy-sidecar", "tls-certificate-rotator-sidecar"];
const SAMPLED_AT = "2026-06-18T09:12:00Z";
const EARLIER_AT = "2026-06-18T09:05:00Z";

function isDetailsPod(summary: ResourceSummary): boolean {
	return summary.kind === "Pod" && summary.namespace === "platform" && summary.name === DETAILS_POD;
}

/** The detail summary carries the controller owner and a restart count that match the container statuses. */
export function detailSummaryFor(summary: ResourceSummary): ResourceSummary {
	return isDetailsPod(summary) ? { ...summary, ownerRef: OWNER, restarts: 1 } : summary;
}

export function detailMetadataFor(summary: ResourceSummary): JsonObject {
	if (!isDetailsPod(summary)) {
		return { labels: { "app.kubernetes.io/name": summary.name }, annotations: { "kubecove.dev/mock": "true" } };
	}
	return {
		name: summary.name,
		namespace: summary.namespace,
		uid: "mock-uid-metrics-gateway-pod",
		resourceVersion: "42",
		ownerReferences: [
			{ apiVersion: "apps/v1", kind: "ReplicaSet", name: OWNER, uid: "mock-uid-metrics-gateway-rs", controller: true },
		],
		labels: {
			"app.kubernetes.io/component": "controller",
			"app.kubernetes.io/instance": "metrics-gateway",
			"app.kubernetes.io/name": "metrics-gateway",
			"pod-template-hash": "9f6d4f7c9",
			team: "platform",
			tier: "infrastructure",
		},
		annotations: {
			"checksum/config": "mock-config-checksum",
			"kubectl.kubernetes.io/last-applied-configuration": '{"apiVersion":"v1"}',
			"owner-team": "platform-infra",
			"prometheus.io/port": "9402",
			"prometheus.io/scrape": "true",
			"reloader.stakater.com/auto": "true",
			"kubecove.dev/notes": "Long mock note that exceeds the inline annotation limit, so it stays in Advanced metadata rather than appearing as an inline badge.",
		},
		managedFields: ["kube-controller-manager", "kubelet", "helm", "kubectl-client-side-apply", "argocd-controller"].map(
			(manager) => ({ manager, operation: "Update" }),
		),
	};
}

export function detailStatusFor(summary: ResourceSummary): JsonObject {
	const base = { phase: summary.status, ready: summary.ready, restarts: summary.restarts };
	if (summary.kind === "Pod" && summary.namespace === "payments" && summary.name === "payments-api-7d9c9b7f8d-k2r9p") {
		return { ...base, containerStatuses: ["app", "sidecar"].map((name) => ({ name, ready: true, restartCount: 0, state: { running: { startedAt: SAMPLED_AT } } })) };
	}
	if (summary.kind === "Pod" && summary.namespace === "argocd" && summary.name === DEX_POD) {
		return {
			...base,
			initContainerStatuses: [{ name: "copy-dex-plugins", ready: true, restartCount: 0, state: { terminated: { reason: "Completed", exitCode: 0, startedAt: EARLIER_AT, finishedAt: EARLIER_AT } } }],
			containerStatuses: DEX_CONTAINERS.map((name) => ({ name, ready: true, restartCount: 0, state: { running: { startedAt: SAMPLED_AT } } })),
		};
	}
	if (!isDetailsPod(summary)) return base;
	return {
		...base,
		initContainerStatuses: [
			{ name: "wait-for-db", ready: true, restartCount: 0, state: { terminated: { reason: "Completed", exitCode: 0, startedAt: EARLIER_AT, finishedAt: EARLIER_AT } } },
		],
		containerStatuses: [
			{
				name: "app",
				ready: true,
				restartCount: 1,
				state: { running: { startedAt: SAMPLED_AT } },
				lastState: { terminated: { reason: "Error", exitCode: 1, startedAt: EARLIER_AT, finishedAt: "2026-06-18T09:08:00Z" } },
			},
			{ name: "sidecar", ready: false, restartCount: 0, state: { waiting: { reason: "CrashLoopBackOff", message: "back-off restarting failed container" } } },
		],
		ephemeralContainerStatuses: [{ name: "debugger", ready: true, restartCount: 0, state: { running: { startedAt: SAMPLED_AT } } }],
	};
}
