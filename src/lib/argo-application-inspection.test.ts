import { describe, expect, test } from "bun:test";
import {
	argoApplicationInspectionQueryOptions,
	argoConnectionStatusQueryOptions,
	buildArgoApplicationInspectionReadSpec,
	buildArgoConnectionStatusReadSpec,
} from "./argo-application-inspection";
import type { ArgoApplicationInspector, ArgoConnectionStatus } from "./gitops-types";
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

function inspection(transport: "connected" | "kubernetes"): ArgoApplicationInspector {
	return {
		application: { name: "guestbook", namespace: "argocd", uid: "uid-1" },
		status: null,
		history: [],
		resources: [],
		comparisons: [],
		conditions: [],
		operationState: null,
		connected: transport === "connected",
		transport,
		provenance: "test",
	};
}

function mockInvokeResult<T>(value: ArgoConnectionStatus | ArgoApplicationInspector): T {
	// SAFETY: each mock returns the exact status or inspection result selected by its query-wrapper test.
	return value as T;
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

	test("keeps healthy profile statuses when another profile fails", async () => {
		const spec = buildArgoConnectionStatusReadSpec({
			profiles,
			clusterContext: "kind-dev",
			workspaceId: "workspace-1",
			kubeconfigEnvVar: "KUBECONFIG",
		});
		const options = argoConnectionStatusQueryOptions(
			{
				invoke: async <T, Args extends object>(_command: string, args?: Args) => {
					if (args && "id" in args && args.id === "primary") throw new Error("profile unavailable");
					return mockInvokeResult<T>(status(true));
				},
			},
			spec,
		);

		expect(await options.queryFn()).toEqual([["backup", status(true)]]);
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
			{ invoke: async <T, Args extends object>(_command: string, _args?: Args) => mockInvokeResult<T>(inspection("kubernetes")) },
			spec,
		);

		expect(options.queryKey).toBe(spec.queryKey);
		expect(options.gcTime).toBe(0);
		expect(options.refetchInterval({ state: { data: inspection("kubernetes") } })).toBe(false);
		expect(options.refetchInterval({ state: { data: inspection("connected") } })).toBe(15_000);
	});
});
