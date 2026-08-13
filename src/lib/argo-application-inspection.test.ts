import { describe, expect, test } from "bun:test";
import {
	argoApplicationInspectionQueryOptions,
	buildArgoApplicationInspectionReadSpec,
	buildArgoConnectionStatusReadSpec,
} from "./argo-application-inspection";
import type { ArgoConnectionStatus } from "./gitops-types";
import type { SavedArgoProfile } from "./settings";

const profiles: SavedArgoProfile[] = [
	{
		id: "primary",
		endpoint: { kind: "externalHttps", url: "https://primary.example.com" },
		clusterContext: "kind-dev",
		workspaceId: "workspace-1",
		kubeconfigSourceKey: "KUBECONFIG",
		rememberCredential: false,
	},
	{
		id: "backup",
		endpoint: { kind: "externalHttps", url: "https://backup.example.com" },
		clusterContext: "kind-dev",
		workspaceId: "workspace-1",
		kubeconfigSourceKey: "KUBECONFIG",
		rememberCredential: false,
	},
];

function status(connected: boolean): ArgoConnectionStatus {
	return {
		profile: null,
		connected,
		username: null,
		unavailableReason: null,
	};
}

function inspectionSpec(
	patch: Partial<Parameters<typeof buildArgoApplicationInspectionReadSpec>[0]> = {},
) {
	return buildArgoApplicationInspectionReadSpec({
		profiles,
		statuses: [
			["primary", status(true)],
			["backup", status(true)],
		],
		statusesPending: false,
		preference: { kind: "automatic" },
		application: { name: "guestbook", namespace: "argocd", uid: "uid-1" },
		clusterContext: "kind-dev",
		workspaceId: "workspace-1",
		kubeconfigEnvVar: "KUBECONFIG",
		redactSecrets: true,
		enabled: true,
		...patch,
	});
}

describe("Argo Application inspection read spec", () => {
	test("preserves saved profile order in shared status identity", () => {
		const spec = buildArgoConnectionStatusReadSpec({
			profiles,
			clusterContext: "kind-dev",
			workspaceId: "workspace-1",
			kubeconfigEnvVar: "KUBECONFIG",
		});

		expect(spec.profiles.map((profile) => profile.id)).toEqual(["primary", "backup"]);
		expect(spec.queryKey.at(-1)).toEqual(["primary", "backup"]);
	});

	test("waits for automatic status resolution before inspection", () => {
		const spec = inspectionSpec({ statuses: undefined, statusesPending: true });

		expect(spec.enabled).toBe(false);
		expect(spec.policy.transport).toBe("kubernetes");
	});

	test("uses first healthy exact-workspace Connected profile", () => {
		const spec = inspectionSpec();

		expect(spec.enabled).toBe(true);
		expect(spec.policy.connectionId).toBe("primary");
		expect(spec.request.transport).toBe("connected");
		expect(spec.queryKey).toContain("connected");
	});

	test("explicit Kubernetes inspection skips pending Connected statuses", () => {
		const spec = inspectionSpec({
			statuses: undefined,
			statusesPending: true,
			preference: { kind: "kubernetes" },
		});

		expect(spec.enabled).toBe(true);
		expect(spec.request.transport).toBe("kubernetes");
	});

	test("explicit unavailable Connected profile never substitutes", () => {
		const spec = inspectionSpec({
			statuses: [["primary", status(true)]],
			preference: { kind: "connected", profileId: "backup" },
		});

		expect(spec.enabled).toBe(false);
		expect(spec.policy.connectionId).toBe("backup");
		expect(spec.policy.unavailable).toBe(true);
	});

	test("keeps fallback under attempted Connected identity and redaction retention", () => {
		const spec = inspectionSpec({ redactSecrets: false });
		const options = argoApplicationInspectionQueryOptions(
			{ invoke: async <T>() => ({ transport: "kubernetes" }) as T },
			spec,
		);

		expect(options.queryKey).toBe(spec.queryKey);
		expect(options.gcTime).toBe(0);
		expect(options.refetchInterval({ state: { data: { transport: "kubernetes" } as never } })).toBe(false);
	});
});
