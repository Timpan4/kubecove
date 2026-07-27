import { describe, expect, test } from "bun:test";
import {
	type ArgoConnectionProfilePolicyInput,
	argoConnectionPreferenceValue,
	argoEndpointIdentity,
	eligibleArgoProfiles,
	normalizeArgoConnectionPreference,
	resolveArgoConnectionPolicy,
	upsertArgoProfileInSavedOrder,
} from "./argo-connection-policy";

const profiles: ArgoConnectionProfilePolicyInput[] = [
	{
		id: "first",
		url: "https://first.example.com",
		clusterContext: "kind-dev",
		workspaceId: "workspace-a",
		kubeconfigSourceKey: "source-a",
	},
	{
		id: "second",
		url: "https://second.example.com",
		clusterContext: "kind-dev",
		workspaceId: "workspace-a",
		kubeconfigSourceKey: "source-a",
	},
	{
		id: "other-workspace",
		url: "https://other.example.com",
		clusterContext: "kind-dev",
		workspaceId: "workspace-b",
		kubeconfigSourceKey: "source-a",
	},
	{
		id: "other-cluster",
		url: "https://cluster.example.com",
		clusterContext: "production",
		workspaceId: "workspace-a",
		kubeconfigSourceKey: "source-a",
	},
	{
		id: "other-source",
		url: "https://source.example.com",
		clusterContext: "kind-dev",
		workspaceId: "workspace-a",
		kubeconfigSourceKey: "source-b",
	},
	{
		id: "legacy-source",
		url: "https://legacy-source.example.com",
		clusterContext: "kind-dev",
		workspaceId: "workspace-a",
	},
	{
		id: "legacy-global",
		url: "https://legacy.example.com",
		clusterContext: "kind-dev",
	},
];

const scope = {
	clusterContext: "kind-dev",
	workspaceId: "workspace-a",
	kubeconfigSourceKey: "source-a",
};

describe("Argo connection policy", () => {
	test("requires exact workspace and cluster eligibility in saved order", () => {
		expect(
			eligibleArgoProfiles(
				profiles,
				scope.clusterContext,
				scope.workspaceId,
				scope.kubeconfigSourceKey,
			).map((profile) => profile.id),
		).toEqual(["first", "second"]);
	});

	test("automatic selects the first healthy eligible profile", () => {
		const choice = resolveArgoConnectionPolicy({
			profiles,
			...scope,
			statuses: [
				["first", { connected: true }],
				["second", { connected: true }],
			],
			preference: { kind: "automatic" },
		});

		expect(choice.transport).toBe("connected");
		expect(choice.connectionId).toBe("first");
		expect(choice.selectedProfile?.id).toBe("first");
		expect(choice.unavailable).toBe(false);
	});

	test("automatic and explicit Kubernetes use Kubernetes when requested", () => {
		const automatic = resolveArgoConnectionPolicy({
			profiles,
			...scope,
			statuses: [["first", { connected: false }]],
		});
		const explicit = resolveArgoConnectionPolicy({
			profiles,
			...scope,
			statuses: [["first", { connected: true }]],
			preference: { kind: "kubernetes" },
		});

		expect(automatic.transport).toBe("kubernetes");
		expect(automatic.connectionId).toBeNull();
		expect(explicit.transport).toBe("kubernetes");
		expect(explicit.connectionId).toBeNull();
	});

	test("explicit Connected keeps its stable ID without substitution", () => {
		const unhealthy = resolveArgoConnectionPolicy({
			profiles,
			...scope,
			statuses: [
				["first", { connected: true }],
				["second", { connected: false }],
			],
			preference: { kind: "connected", profileId: "second" },
		});
		const missing = resolveArgoConnectionPolicy({
			profiles,
			...scope,
			statuses: [["first", { connected: true }]],
			preference: { kind: "connected", profileId: "missing" },
		});

		expect(unhealthy.connectionId).toBe("second");
		expect(unhealthy.selectedProfile?.id).toBe("second");
		expect(unhealthy.unavailable).toBe(true);
		expect(missing.connectionId).toBe("missing");
		expect(missing.selectedProfile).toBeNull();
		expect(missing.unavailable).toBe(true);
	});

	test("reconnect preserves saved order and stable IDs", () => {
		const reconnected = upsertArgoProfileInSavedOrder(
			profiles,
			{ ...profiles[0], url: "https://updated.example.com" },
			"first",
		);
		const added = upsertArgoProfileInSavedOrder(reconnected, {
			id: "third",
			url: "https://third.example.com",
			clusterContext: "kind-dev",
			workspaceId: "workspace-a",
		});

		expect(reconnected.map((profile) => profile.id)).toEqual(
			profiles.map((profile) => profile.id),
		);
		expect(reconnected[0]?.url).toBe("https://updated.example.com");
		expect(added.at(-1)?.id).toBe("third");
	});

	test("identifies external and private service endpoints stably", () => {
		expect(argoEndpointIdentity({ kind: "externalHttps", url: " HTTPS://ARGO.EXAMPLE.COM " })).toBe(
			"https://argo.example.com",
		);
		expect(
			argoEndpointIdentity({
				kind: "serviceTunnel",
				namespace: " ArgoCD ",
				serviceName: " ArgoCD-Server ",
				servicePort: 443,
				scheme: "https",
				rootPath: " /argo-cd ",
				tlsServerName: " ARGO.INTERNAL ",
			}),
		).toBe("serviceTunnel:argocd:argocd-server:443:https:/argo-cd:argo.internal");
	});

	test("serializes and validates stable Connected profile IDs", () => {
		const value = argoConnectionPreferenceValue({
			kind: "connected",
			profileId: "argo:workspace-a:kind-dev:https://argo.example.com",
		});

		expect(value).toBe(
			"connected:argo:workspace-a:kind-dev:https://argo.example.com",
		);
		expect(normalizeArgoConnectionPreference(value)).toEqual({
			kind: "connected",
			profileId: "argo:workspace-a:kind-dev:https://argo.example.com",
		});
		expect(
			normalizeArgoConnectionPreference({ kind: "connected", profileId: " " }),
		).toEqual({ kind: "automatic" });
	});
});
