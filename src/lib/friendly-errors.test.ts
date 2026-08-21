import {
	friendlyError,
	friendlyErrorBucket,
	messageFromFriendlyError,
} from "./friendly-errors";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void | Promise<void>): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
};
declare const Bun: {
	file(path: string): { text(): Promise<string> };
};

describe("friendlyError", () => {
	const cases: Array<[string, unknown, string]> = [
		["kubeconfig", { kind: "kubeconfig", message: "missing config" }, "kubeconfigConfig"],
		["forbidden", { kind: "forbidden", message: "pods is forbidden" }, "forbiddenRbac"],
		["not found", { kind: "notFound", message: "pods x not found" }, "notFoundStale"],
		["validation", { kind: "validation", message: "namespace is required" }, "validation"],
		["serialization", { kind: "serialization", message: "could not serialize" }, "serialization"],
		[
			"admission",
			{ kind: "admissionDenied", message: "admission webhook denied the request" },
			"admissionPolicy",
		],
		[
			"immutable",
			{ kind: "immutableField", message: "pod updates may not change fields other than spec" },
			"immutableField",
		],
		[
			"field manager",
			{ kind: "fieldManagerConflict", message: "Apply failed with conflicts" },
			"fieldManagerConflict",
		],
		[
			"live session",
			{ kind: "cluster", message: "no ready pod matched service selector" },
			"liveSessionTargetUnavailable",
		],
		[
			"provider",
			{ kind: "cluster", message: "metrics.k8s.io discovery unavailable" },
			"providerDiscoveryUnavailable",
		],
		[
			"network",
			{ kind: "cluster", message: "connection refused while connecting to API server" },
			"networkTransient",
		],
		[
			"service connect",
			{ kind: "cluster", message: "ServiceError: client error (Connect)" },
			"networkTransient",
		],
		[
			"authentication",
			{ kind: "cluster", message: "Unauthorized: authentication required" },
			"authentication",
		],
	];

	for (const [name, error, bucket] of cases) {
		test(`maps ${name} errors`, () => {
			expect(friendlyErrorBucket(error)).toBe(bucket);
		});
	}

	test("keeps unknown technical detail copyable", () => {
		const message = "ApiError: unexpected nested status";
		const presentation = friendlyError(message);

		expect(presentation.bucket).toBe("unknown");
		expect(presentation.title).toBe("KubeCove could not simplify this error yet");
		expect(presentation.copyText).toBe(message);
	});

	test("redacts credentials from technical and copied detail", () => {
		const detail = messageFromFriendlyError(
			"request https://admin:secret@api.example.test?token=abc&x-amz-signature=deadbeef Authorization: Bearer eyJ.secret X-Api-Key: api-secret",
		);

		expect(detail.includes("admin:secret")).toBe(false);
		expect(detail.includes("token=abc")).toBe(false);
		expect(detail.includes("deadbeef")).toBe(false);
		expect(detail.includes("eyJ.secret")).toBe(false);
		expect(detail.includes("api-secret")).toBe(false);
		expect(detail.includes("[REDACTED]")).toBe(true);
	});

	test("redacts basic authorization credentials", () => {
		const detail = messageFromFriendlyError("Authorization: Basic dXNlcjpwYXNz");

		expect(detail).toBe("Authorization: Basic [REDACTED]");
	});

	test("uses neutral recovery guidance for mixed workspace failures", () => {
		const presentation = friendlyError(
			{
				kind: "mixedWorkspaceConnection",
				message:
					'Context "dev": Unauthorized\nContext "prod": ServiceError: client error (Connect)',
			},
			{ operation: "resourcesLoad", target: "saved contexts" },
		);

		expect(presentation.bucket).toBe("mixedWorkspaceConnection");
		expect(presentation.summary).toBe(
			"KubeCove could not load resources for saved contexts; each context's cause is listed in the technical detail.",
		);
	});

	test("uses compact partial tone", () => {
		const presentation = friendlyError(
			{ kind: "forbidden", message: "events is forbidden" },
			{ operation: "eventsLoad", partial: true },
		);

		expect(presentation.tone).toBe("warning");
		expect(presentation.title).toBe("Some events could not load");
	});

	test("names connection target and recovery path", () => {
		const presentation = friendlyError(
			{ kind: "cluster", message: "ServiceError: client error (Connect)" },
			{ operation: "resourcesLoad", target: 'context "kind-prod"' },
		);

		expect(presentation.summary).toBe(
			'The connection failed while loading resources for context "kind-prod".',
		);
		expect(presentation.next).toBe(
			"Check the selected context's API endpoint, network or VPN path, and TLS certificate trust, then retry.",
		);
	});

	test("component keeps technical detail collapsed and copyable", async () => {
		const source = await Bun.file("src/components/FriendlyError.svelte").text();

		expect(source.includes("<details")).toBe(true);
		expect(source.includes("<details open")).toBe(false);
		expect(source.includes("navigator.clipboard.writeText")).toBe(true);
		expect(source.includes('!compact && presentation.next')).toBe(true);
	});
});
