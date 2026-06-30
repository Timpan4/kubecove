import { friendlyError, friendlyErrorBucket } from "./friendly-errors";

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

	test("uses compact partial tone", () => {
		const presentation = friendlyError(
			{ kind: "forbidden", message: "events is forbidden" },
			{ operation: "eventsLoad", partial: true },
		);

		expect(presentation.tone).toBe("warning");
		expect(presentation.title).toBe("Some events could not load");
	});

	test("component keeps technical detail collapsed and copyable", async () => {
		const source = await Bun.file("src/components/FriendlyError.svelte").text();

		expect(source.includes("<details")).toBe(true);
		expect(source.includes("<details open")).toBe(false);
		expect(source.includes("navigator.clipboard.writeText")).toBe(true);
		expect(source.includes('!compact && presentation.next')).toBe(true);
	});
});
