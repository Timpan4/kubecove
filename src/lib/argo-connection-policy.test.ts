import { describe, expect, test } from "bun:test";
import {
	type ArgoConnectionProfilePolicyInput,
	argoConnectionPreferenceValue,
	argoEndpointIdentity,
	eligibleArgoProfiles,
	normalizeArgoConnectionPreference,
	resolveArgoConnectionPolicy,
	savedArgoProfileId,
	upsertArgoProfileInSavedOrder,
} from "./argo-connection-policy";
import type { ArgoServerEndpoint } from "./gitops-types";

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

	test("identifies external endpoints by their case-insensitive scheme and host only", () => {
		const identity = (url: string) => argoEndpointIdentity({ kind: "externalHttps", url });
		expect(identity(" HTTPS://ARGO.EXAMPLE.COM ")).toBe("externalHttps:https://argo.example.com");
		expect(identity("https://Argo.Example.com:8443/Argo/Apps")).toBe(
			"externalHttps:https://argo.example.com:8443/Argo/Apps",
		);
		// Two servers that differ only in path case are two endpoints (#662).
		expect(identity("https://argo.example.com/Argo")).not.toBe(identity("https://argo.example.com/argo"));
		// The host ends at the first path, query or fragment delimiter.
		expect(identity("https://ARGO.example.com?Q")).toBe("externalHttps:https://argo.example.com?Q");
		expect(identity("https://ARGO.example.com#F")).toBe("externalHttps:https://argo.example.com#F");
		// Without a scheme separator there is no host to find, so nothing is lowercased.
		expect(identity("Argo.Example.com/Argo")).toBe("externalHttps:Argo.Example.com/Argo");
	});

	test("never makes a new external identity equal to one stored before path case counted", () => {
		// Identities made before lowercased the whole URL, so saved ids end in it (ADR 0013).
		const before = "https://argo.example.com/argo";
		expect(argoEndpointIdentity({ kind: "externalHttps", url: "https://argo.example.com/argo" })).not.toBe(before);
	});

	test("reuses the id of a saved profile with the same endpoint, whatever rule made it", () => {
		const saved: { id: string; endpoint: ArgoServerEndpoint }[] = [
			// Made before path case counted: the id lowercases this profile's /Argo path.
			{ id: "argo:w:c:global:https://argo.example.com/argo", endpoint: { kind: "externalHttps", url: "https://argo.example.com/Argo" } },
			{ id: "tunnel", endpoint: { kind: "serviceTunnel", namespace: "argocd", serviceName: "argocd-server", servicePort: 443, scheme: "https" } },
		];
		expect(savedArgoProfileId(saved, { kind: "externalHttps", url: "https://ARGO.example.com/Argo" })).toBe(saved[0]?.id);
		// The other server, whose new id could otherwise take the old profile's id.
		expect(savedArgoProfileId(saved, { kind: "externalHttps", url: "https://argo.example.com/argo" })).toBeUndefined();
		expect(savedArgoProfileId(saved, { kind: "serviceTunnel", namespace: "ArgoCD", serviceName: "argocd-server", servicePort: 443, scheme: "https" })).toBe("tunnel");
	});

	test("matches a saved profile by the URL the backend saved, not the URL typed for it", () => {
		// The backend saves the canonical URL: no trailing slash, default port, query or fragment.
		const saved: { id: string; endpoint: ArgoServerEndpoint }[] = [
			{ id: "argo:w:c:global:https://argocd.example.com:443/argo/", endpoint: { kind: "externalHttps", url: "https://argocd.example.com/Argo" } },
		];
		const typed = (url: string) => savedArgoProfileId(saved, { kind: "externalHttps", url });
		expect(typed(" https://ArgoCD.example.com:443/Argo/ ")).toBe(saved[0]?.id);
		expect(typed("https://argocd.example.com/Argo?x=1#y")).toBe(saved[0]?.id);
		// Canonical URLs keep path case, so the other server still gets its own profile.
		expect(typed("https://argocd.example.com:443/argo/")).toBeUndefined();
		// A URL the backend cannot parse matches by its typed identity only.
		expect(typed("argocd.example.com/Argo")).toBeUndefined();
	});

	test("identifies private service endpoints as before", () => {
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
