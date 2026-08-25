import {
	argoListInvalidationKey,
	createArgoListFreshness,
} from "./argo-application-freshness";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void | Promise<void>): void;
declare function expect<T>(actual: T): {
	toBe(expected: T): void;
	toEqual(expected: T): void;
};

const changed = (kind: string) => ({
	type: "resourceChanged" as const,
	streamId: "watch-1",
	action: "MODIFIED" as const,
	target: { cluster: "kind-dev", kind, namespace: "argocd", name: "guestbook" },
});

const waitForDebounce = () => new Promise((resolve) => setTimeout(resolve, 300));

describe("Argo application freshness", () => {
	test("maps watched Argo kinds to their exact list keys", () => {
		for (const [kind, key] of [
			["Application", "argo-apps"],
			["ApplicationSet", "argo-appsets"],
			["AppProject", "argo-appprojects"],
		] as const) {
			expect(argoListInvalidationKey(changed(kind), "KUBECONFIG")).toEqual([
				key,
				"kubeconfigEnv=KUBECONFIG",
				"kind-dev",
			]);
		}
	});

	test("invalidates matching watch events and ignores unrelated events", async () => {
		const invalidated: (readonly unknown[])[] = [];
		const freshness = createArgoListFreshness(
			(queryKey) => invalidated.push(queryKey),
			"KUBECONFIG",
		);
		freshness.handle(changed("Application"));
		freshness.handle(changed("ApplicationSet"));
		freshness.handle(changed("AppProject"));
		freshness.handle(changed("Deployment"));
		await waitForDebounce();

		expect(invalidated).toEqual([
			["argo-apps", "kubeconfigEnv=KUBECONFIG", "kind-dev"],
			["argo-appsets", "kubeconfigEnv=KUBECONFIG", "kind-dev"],
			["argo-appprojects", "kubeconfigEnv=KUBECONFIG", "kind-dev"],
		]);
	});

	test("coalesces debounce bursts", async () => {
		const invalidated: (readonly unknown[])[] = [];
		const freshness = createArgoListFreshness((queryKey) => invalidated.push(queryKey));
		freshness.handle(changed("Application"));
		freshness.handle(changed("Application"));
		freshness.handle(changed("Application"));
		await waitForDebounce();

		expect(invalidated).toEqual([["argo-apps", "kubeconfigEnv=KUBECONFIG", "kind-dev"]]);
	});

	test("cleanup suppresses scheduled and late watch events", async () => {
		const invalidated: (readonly unknown[])[] = [];
		const freshness = createArgoListFreshness((queryKey) => invalidated.push(queryKey));
		freshness.handle(changed("Application"));
		freshness.dispose();
		freshness.handle(changed("ApplicationSet"));
		await waitForDebounce();

		expect(invalidated).toEqual([]);
	});
});
