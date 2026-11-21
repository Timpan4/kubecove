// kube-rs port-forward protocol fixture shared by server and native desktop E2E.
// It echoes bytes and never creates Kubernetes resources.
export function startPortForwardFixture() {
	const fixturePod = (name: string, resourceVersion: string) => ({
		apiVersion: "v1", kind: "Pod", metadata: { name, namespace: "readable", resourceVersion },
		spec: { containers: [] }, status: { phase: "Running" },
	});
	const initialPod = fixturePod("readable-pod", "1");
	const addedPod = fixturePod("watch-only-pod", "2");
	let podAdded = false;
	return Bun.serve<{ port: number; mode: string }>({
		hostname: "127.0.0.1", port: 0,
		fetch(request, server) {
			const url = new URL(request.url);
			if (url.pathname === "/api/v1/namespaces") {
				return Response.json({ apiVersion: "v1", kind: "NamespaceList", metadata: {}, items: [
					{ metadata: { name: "readable" }, status: { phase: "Active" } },
				] });
			}
			if (url.pathname === "/api/v1/secrets" || url.pathname === "/api/v1/namespaces/denied/pods") {
				return Response.json({ apiVersion: "v1", kind: "Status", status: "Failure", reason: "Forbidden", message: "Fixture account cannot list this resource", code: 403 }, { status: 403 });
			}
			if (url.pathname === "/api/v1/pods" || url.pathname === "/api/v1/namespaces/readable/pods") {
				if (url.searchParams.get("watch") === "true") {
					podAdded = true;
					// Kubernetes may end a watch after delivering events. A resume at version 2 has no new events.
					const event = url.searchParams.get("resourceVersion") === "2" ? "" : `${JSON.stringify({ type: "ADDED", object: addedPod })}\n`;
					return new Response(event, { headers: { "Content-Type": "application/json" } });
				}
				return Response.json({ apiVersion: "v1", kind: "PodList", metadata: { resourceVersion: podAdded ? "2" : "1" },
					items: podAdded ? [initialPod, addedPod] : [initialPod] });
			}
			if (url.pathname === "/api/v1/namespaces/unauthorized/pods") {
				return Response.json({ apiVersion: "v1", kind: "Status", status: "Failure", reason: "Unauthorized", message: "Fixture authentication failed", code: 401 }, { status: 401 });
			}
			if (url.pathname === "/api/v1/namespaces/default/services/forward-service") {
				return Response.json({ apiVersion: "v1", kind: "Service", metadata: { name: "forward-service", namespace: "default" },
					spec: { selector: { app: "forward" }, ports: [{ port: 80, targetPort: "http", protocol: "TCP" }] } });
			}
			if (url.pathname === "/api/v1/namespaces/default/pods" && url.searchParams.get("labelSelector") === "app=forward") {
				return Response.json({ apiVersion: "v1", kind: "PodList", metadata: {}, items: [{
					metadata: { name: "forward-target", namespace: "default", labels: { app: "forward" } },
					spec: { containers: [{ name: "app", image: "fixture", ports: [{ name: "http", containerPort: 8080, protocol: "TCP" }] }] },
					status: { phase: "Running", conditions: [{ type: "Ready", status: "True" }] },
				}] });
			}
			const pod = /^\/api\/v1\/namespaces\/default\/pods\/(forward-target|forward-close|forward-error)\/portforward$/.exec(url.pathname)?.[1];
			if (pod) {
				if (server.upgrade(request, { data: { port: 8080, mode: pod }, headers: { "Sec-WebSocket-Protocol": "v4.channel.k8s.io" } })) return;
			}
			return new Response("Not found", { status: 404 });
		},
		websocket: {
			open(socket) {
				// kube-rs requires little-endian port initialization on data/error channels.
				for (const channel of [0, 1]) socket.send(new Uint8Array([channel, socket.data.port & 0xff, socket.data.port >> 8]));
			},
			message(socket, message) {
				if (message instanceof Uint8Array && message[0] === 0) {
					if (socket.data.mode === "forward-error") {
						const error = new TextEncoder().encode("fixture pod refused connection");
						const frame = new Uint8Array(1 + error.length);
						frame[0] = 1;
						frame.set(error, 1);
						socket.send(frame);
					} else {
						socket.send(message);
						if (socket.data.mode === "forward-close") socket.close();
					}
				}
			},
		},
	});
}

if (import.meta.main) {
	const server = startPortForwardFixture();
	console.log(server.url.toString());
	for (const signal of ["SIGINT", "SIGTERM"] as const) process.once(signal, () => server.stop(true));
}
