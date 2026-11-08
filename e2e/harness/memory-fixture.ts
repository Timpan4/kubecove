import { chmod, readFile } from "node:fs/promises";
import { join } from "node:path";
import { resources } from "../../src/lib/tauri-dev-mock-data";

export const MEMORY_FIXTURE_ROWS = {
	normal: resources.length,
	// Matches the resource-metrics workload in scripts/perf-frontend.ts.
	large: 10_000,
} as const;
export type MemoryFixture = keyof typeof MEMORY_FIXTURE_ROWS;

/** Measurement options. `latencyMs` delays every response, as a cluster round trip would. */
export type MemoryFixtureOptions = { latencyMs?: number };
/** One served request, for counting the reads a client action makes. */
export type FixtureRequest = { epochMs: number; method: string; path: string; watch: boolean };

export async function startMemoryFixture(size: MemoryFixture, directory: string, { latencyMs = 0 }: MemoryFixtureOptions = {}) {
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
	// Open Pod watch streams. They stay idle unless a benchmark calls `burst`.
	const podWatches = new Set<ReadableStreamDefaultController<Uint8Array>>();
	let resourceVersion = 1;
	const requests: FixtureRequest[] = [];
	const server = Bun.serve({
		hostname: "127.0.0.1", port: 0,
		// Watches intentionally remain idle until the owned fixture is stopped.
		idleTimeout: 0,
		tls: { key: Bun.file(key), cert: Bun.file(cert) },
		async fetch(request) {
			const url = new URL(request.url);
			requests.push({ epochMs: Date.now(), method: request.method, path: `${url.pathname}${url.search}`, watch: url.searchParams.get("watch") === "true" });
			if (latencyMs > 0) await Bun.sleep(latencyMs);
			if (request.method !== "GET") return Response.json({ kind: "Status", status: "Failure", reason: "MethodNotAllowed", code: 405 }, { status: 405 });
			const aggregatedDiscovery = request.headers.get("accept")?.split(",").some((media) => media.split(";").some((parameter) => parameter.trim() === "g=apidiscovery.k8s.io"));
			if ((url.pathname === "/api" || url.pathname === "/apis") && aggregatedDiscovery) return Response.json({ apiVersion: "apidiscovery.k8s.io/v2", kind: "APIGroupDiscoveryList", metadata: {}, items: aggregated[url.pathname] });
			const apiDiscovery = discovery.get(url.pathname);
			if (apiDiscovery) return Response.json(apiDiscovery);
			if (url.searchParams.get("watch") === "true") {
				// The watch stays idle unless `burst` is called; the loaded fixture never changes otherwise.
				const podWatch = !url.pathname.includes("/metrics.k8s.io/") && url.pathname.split("/").filter(Boolean).at(-1) === "pods";
				let owned: ReadableStreamDefaultController<Uint8Array> | undefined;
				return new Response(new ReadableStream<Uint8Array>({
					start(controller) {
						controller.enqueue(new TextEncoder().encode(`${JSON.stringify({ type: "BOOKMARK", object: { apiVersion: "v1", kind: "Pod", metadata: { resourceVersion: "1" } } })}\n`));
						if (podWatch) { owned = controller; podWatches.add(controller); }
					},
					cancel() { if (owned) podWatches.delete(owned); },
				}), { headers: { "content-type": "application/json" } });
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
	/** Sends `events` MODIFIED Pod events (at most one per Pod), with `restarts` as the restart count, to every open Pod watch. */
	function burst(restarts: number, events = pods.length) {
		resourceVersion += 1;
		const lines = pods.slice(0, events).map((pod) => JSON.stringify({ type: "MODIFIED", object: { ...pod, metadata: { ...pod.metadata, resourceVersion: String(resourceVersion) }, status: { ...pod.status, containerStatuses: pod.status.containerStatuses.map((status) => ({ ...status, restartCount: restarts })) } } }));
		const chunk = new TextEncoder().encode(`${lines.join("\n")}\n`);
		for (const controller of podWatches) {
			try { controller.enqueue(chunk); } catch { podWatches.delete(controller); }
		}
		return { watches: podWatches.size, events: lines.length };
	}
	return { url: `https://127.0.0.1:${server.port}`, authority, rows: count, burst, requests, stop: () => server.stop(true) };
}
