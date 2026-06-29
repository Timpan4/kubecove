import { bench, describe } from "vitest";
import { buildYamlDryRunDiff } from "@/features/resource-detail/yamlTabDiff";

function buildManifest(replicas: number, version: string): string {
	const containers = Array.from({ length: 40 }, (_, index) => {
		const name = `worker-${index}`;
		return [
			`        - name: ${name}`,
			`          image: ghcr.io/kubecove/${name}:${version}`,
			"          env:",
			`            - name: WORKER_INDEX`,
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
		"spec:",
		`  replicas: ${replicas}`,
		"  template:",
		"    spec:",
		"      containers:",
		...containers,
	].join("\n");
}

const currentYaml = buildManifest(3, "1.0.0");
const dryRunYaml = buildManifest(5, "1.1.0");

describe("yaml dry-run diff", () => {
	bench("buildYamlDryRunDiff (compact git style)", () => {
		buildYamlDryRunDiff({
			currentYaml,
			dryRunYaml,
			style: "git",
			full: false,
			forceConflicts: false,
		});
	});

	bench("buildYamlDryRunDiff (full clean style)", () => {
		buildYamlDryRunDiff({
			currentYaml,
			dryRunYaml,
			style: "clean",
			full: true,
			forceConflicts: true,
		});
	});
});
