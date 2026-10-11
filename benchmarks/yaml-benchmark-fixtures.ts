export function buildYamlManifest(containerCount: number): string {
	const containers = Array.from({ length: containerCount }, (_, index) => {
		const name = `worker-${index}`;
		return [
			`        - name: ${name}`,
			`          image: ghcr.io/kubecove/${name}:1.2.0`,
			"          env:",
			"            - name: WORKER_INDEX",
			`              value: "${index}"`,
			"          resources:",
			"            requests:",
			`              cpu: "${100 + index}m"`,
			"              memory: 128Mi",
			"            limits:",
			"              memory: 256Mi",
		].join("\n");
	});
	return [
		"apiVersion: apps/v1",
		"kind: Deployment",
		"metadata:",
		"  name: checkout-api",
		"  namespace: checkout",
		"  labels:",
		"    app.kubernetes.io/name: checkout-api",
		"spec:",
		"  replicas: 4",
		"  selector:",
		"    matchLabels:",
		"      app.kubernetes.io/name: checkout-api",
		"  template:",
		"    metadata:",
		"      labels:",
		"        app.kubernetes.io/name: checkout-api",
		"    spec:",
		"      containers:",
		...containers,
	].join("\n");
}
