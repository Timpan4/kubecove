import { chmod, readFile } from "node:fs/promises";
import { join } from "node:path";
import { resources } from "../../src/lib/tauri-dev-mock-data";

export const MEMORY_FIXTURE_ROWS = {
	normal: resources.length,
	// Matches the resource-metrics workload in scripts/perf-frontend.ts.
	large: 10_000,
} as const;
export type MemoryFixture = keyof typeof MEMORY_FIXTURE_ROWS;

export async function startMemoryFixture(size: MemoryFixture, directory: string) {
	const key = join(directory, "key.pem");
	const cert = join(directory, "cert.pem");
	const certificate = Bun.spawn(["openssl", "req", "-x509", "-newkey", "rsa", "-nodes", "-keyout", key, "-out", cert, "-subj", "/CN=localhost", "-addext", "subjectAltName=IP:127.0.0.1", "-addext", "basicConstraints=critical,CA:FALSE"], { stdout: "ignore", stderr: "pipe" });
	const [code, error] = await Promise.all([certificate.exited, new Response(certificate.stderr).text()]);
	if (code !== 0) {
		throw new Error(`Fixture certificate generation failed: ${error}`);
	}
	await chmod(key, 0o600);
	const authority = (await readFile(cert)).toString("base64");
	const count = MEMORY_FIXTURE_ROWS[size];
	const metadata = (name: string) => ({ name, namespace: "profile", uid: name, resourceVersion: "1", creationTimestamp: "2026-01-01T00:00:00Z" });
	const pods = Array.from({ length: count }, (_, index) => ({
		apiVersion: "v1", kind: "Pod",
		metadata: { ...metadata(`profile-pod-${index}`), labels: { app: "profile" }, ownerReferences: [{ apiVersion: "apps/v1", kind: "ReplicaSet", name: "profile-rs", uid: "profile-rs", controller: true }] },
		spec: { containers: [{ name: "app", image: "fixture", ports: [{ containerPort: 8080 }] }], nodeName: "profile-node" },
		status: { phase: "Running", conditions: [{ type: "Ready", status: "True" }], containerStatuses: [{ name: "app", ready: true, restartCount: 0, image: "fixture", imageID: "fixture", state: { running: { startedAt: "2026-01-01T00:00:00Z" } } }] },
	}));
	const template = { metadata: { labels: { app: "profile" } }, spec: { containers: [{ name: "app", image: "fixture" }] } };
	const workloads = {
		deployments: [{ apiVersion: "apps/v1", kind: "Deployment", metadata: metadata("profile-deployment"), spec: { replicas: count, selector: { matchLabels: { app: "profile" } }, template }, status: { replicas: count, readyReplicas: count, availableReplicas: count } }],
		replicasets: [{ apiVersion: "apps/v1", kind: "ReplicaSet", metadata: { ...metadata("profile-rs"), ownerReferences: [{ apiVersion: "apps/v1", kind: "Deployment", name: "profile-deployment", uid: "profile-deployment", controller: true }] }, spec: { replicas: count, selector: { matchLabels: { app: "profile" } }, template }, status: { replicas: count, readyReplicas: count } }],
	};
	const collections = new Map(Object.entries({
		pods, ...workloads,
		services: [{ apiVersion: "v1", kind: "Service", metadata: metadata("profile-service"), spec: { selector: { app: "profile" }, ports: [{ port: 80, targetPort: 8080 }], type: "ClusterIP", clusterIP: "10.0.0.1" } }],
		namespaces: [{ apiVersion: "v1", kind: "Namespace", metadata: { name: "profile", uid: "profile", resourceVersion: "1" }, status: { phase: "Active" } }],
		nodes: [{ apiVersion: "v1", kind: "Node", metadata: { name: "profile-node", uid: "profile-node", resourceVersion: "1" }, status: { capacity: { cpu: "2", memory: "2Gi" }, conditions: [{ type: "Ready", status: "True" }] } }],
	}));
	const metrics = pods.map((pod) => ({ apiVersion: "metrics.k8s.io/v1beta1", kind: "PodMetrics", metadata: pod.metadata, timestamp: "2026-01-01T00:00:00Z", window: "30s", containers: [{ name: "app", usage: { cpu: "10m", memory: "1024Ki" } }] }));
	const groups = ["apps/v1", "metrics.k8s.io/v1beta1"].map((groupVersion) => {
		const [name, version] = groupVersion.split("/");
		return { name, versions: [{ groupVersion, version }], preferredVersion: { groupVersion, version } };
	});
	const apiResource = (name: string, kind: string, namespaced: boolean) => ({ name, singularName: "", kind, namespaced, verbs: ["get", "list", "watch"] });
	const discovery = new Map(Object.entries({
		"/api": { apiVersion: "v1", kind: "APIVersions", versions: ["v1"], serverAddressByClientCIDRs: [] },
		"/apis": { apiVersion: "v1", kind: "APIGroupList", groups },
		"/api/v1": { apiVersion: "v1", kind: "APIResourceList", groupVersion: "v1", resources: [apiResource("pods", "Pod", true), apiResource("services", "Service", true), apiResource("namespaces", "Namespace", false), apiResource("nodes", "Node", false)] },
		"/apis/apps/v1": { apiVersion: "v1", kind: "APIResourceList", groupVersion: "apps/v1", resources: [apiResource("deployments", "Deployment", true), apiResource("replicasets", "ReplicaSet", true)] },
		"/apis/metrics.k8s.io/v1beta1": { apiVersion: "v1", kind: "APIResourceList", groupVersion: "metrics.k8s.io/v1beta1", resources: [apiResource("pods", "PodMetrics", true), apiResource("nodes", "NodeMetrics", false)] },
	}));
	const aggregatedGroup = (name: string, version: string, resources: ReturnType<typeof apiResource>[]) => ({
		metadata: { name }, versions: [{ version, freshness: "Current", resources: resources.map((resource) => ({ resource: resource.name, responseKind: { group: name, version, kind: resource.kind }, scope: resource.namespaced ? "Namespaced" : "Cluster", singularResource: "", verbs: resource.verbs })) }],
	});
	const aggregated = {
		"/api": [aggregatedGroup("", "v1", [apiResource("pods", "Pod", true), apiResource("services", "Service", true), apiResource("namespaces", "Namespace", false), apiResource("nodes", "Node", false)])],
		"/apis": [aggregatedGroup("apps", "v1", [apiResource("deployments", "Deployment", true), apiResource("replicasets", "ReplicaSet", true)]), aggregatedGroup("metrics.k8s.io", "v1beta1", [apiResource("pods", "PodMetrics", true), apiResource("nodes", "NodeMetrics", false)])],
	};
	const server = Bun.serve({
		hostname: "127.0.0.1", port: 0,
		// Watches intentionally remain idle until the owned fixture is stopped.
		idleTimeout: 0,
		tls: { key: Bun.file(key), cert: Bun.file(cert) },
		fetch(request) {
			if (request.method !== "GET") return Response.json({ kind: "Status", status: "Failure", reason: "MethodNotAllowed", code: 405 }, { status: 405 });
			const url = new URL(request.url);
			if ((url.pathname === "/api" || url.pathname === "/apis") && request.headers.get("accept")?.includes("apidiscovery.k8s.io")) return Response.json({ apiVersion: "apidiscovery.k8s.io/v2", kind: "APIGroupDiscoveryList", metadata: {}, items: aggregated[url.pathname] });
			const apiDiscovery = discovery.get(url.pathname);
			if (apiDiscovery) return Response.json(apiDiscovery);
			if (url.searchParams.get("watch") === "true") {
				// Keep the read-only watch idle; no synthetic events alter the loaded fixture.
				return new Response(new ReadableStream({ start(controller) { controller.enqueue(new TextEncoder().encode(`${JSON.stringify({ type: "BOOKMARK", object: { apiVersion: "v1", kind: "Pod", metadata: { resourceVersion: "1" } } })}\n`)); } }), { headers: { "content-type": "application/json" } });
			}
			const segments = url.pathname.split("/").filter(Boolean);
			const last = segments.at(-1) ?? "";
			const plural = segments.at(-2) ?? "";
			const items = url.pathname.includes("/metrics.k8s.io/") ? (last === "pods" ? metrics : []) : collections.get(last) ?? [];
			const namedCollection = collections.get(plural);
			if (namedCollection && !collections.has(last)) {
				const value = namedCollection.find((item) => item.metadata.name === last);
				if (value) return Response.json(value);
				return Response.json({ kind: "Status", status: "Failure", reason: "NotFound", code: 404 }, { status: 404 });
			}
			return Response.json({ apiVersion: "v1", kind: "List", metadata: { resourceVersion: "1" }, items });
		},
	});
	return { url: `https://127.0.0.1:${server.port}`, authority, rows: count, stop: () => server.stop(true) };
}
