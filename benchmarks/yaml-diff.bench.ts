import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	buildYamlDryRunDiff,
	diffLineClassName,
} from "@/features/resource-detail/yamlTabDiff";

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
const identicalYaml = `${currentYaml}\n`;
const sparseDryRunYaml = currentYaml
	.replace("replicas: 3", "replicas: 4")
	.replace(
		"image: ghcr.io/kubecove/worker-20:1.0.0",
		"image: ghcr.io/kubecove/worker-20:1.1.0",
	);
const insertedContainer = [
	"        - name: migration",
	"          image: ghcr.io/kubecove/migration:1.0.0",
	"          env:",
	"            - name: WORKER_INDEX",
	"              value: migration",
	"          resources:",
	"            requests:",
	"              cpu: 1m",
	"              memory: 32Mi",
	"            limits:",
	"              memory: 64Mi",
].join("\n");
const structuralCurrentYaml = buildManifest(3, "1.0.0");
const structuralDryRunYaml = structuralCurrentYaml.replace(
	/\n {8}- name: worker-39[\s\S]*$/,
	`\n${insertedContainer}`,
);
const compactDiff = buildYamlDryRunDiff({
	currentYaml,
	dryRunYaml,
	style: "git",
	full: false,
	forceConflicts: false,
});
const sparseDiff = buildYamlDryRunDiff({
	currentYaml,
	dryRunYaml: sparseDryRunYaml,
	style: "git",
	full: false,
	forceConflicts: false,
});
const structuralDiff = buildYamlDryRunDiff({
	currentYaml: structuralCurrentYaml,
	dryRunYaml: structuralDryRunYaml,
	style: "git",
	full: true,
	forceConflicts: false,
});
const identicalDiff = buildYamlDryRunDiff({
	currentYaml: identicalYaml,
	dryRunYaml: identicalYaml,
	style: "clean",
	full: false,
	forceConflicts: false,
});

assert(currentYaml.includes("worker-39"));
assert(compactDiff.some((line) => line.type === "add"));
assert(compactDiff.some((line) => line.type === "remove"));
assert(
	sparseDiff.some((line) => line.text === '-          image: ghcr.io/kubecove/worker-20:1.0.0') &&
		sparseDiff.some((line) => line.text === '+          image: ghcr.io/kubecove/worker-20:1.1.0'),
);
assert(
	structuralDiff.some((line) => line.text === "-        - name: worker-39") &&
		structuralDiff.some((line) => line.text === "+        - name: migration"),
);
assert(identicalDiff.some((line) => line.type === "empty"));

describe("F16 active YAML comparison branches and presentation (40 containers)", () => {
	bench("F16 compact git diff + production 24-line class projection", () => {
		buildYamlDryRunDiff({
			currentYaml,
			dryRunYaml,
			style: "git",
			full: false,
			forceConflicts: false,
		})
			.slice(0, 24)
			.map((line) => diffLineClassName(line.type));
		buildYamlDryRunDiff({
			currentYaml,
			dryRunYaml: sparseDryRunYaml,
			style: "git",
			full: false,
			forceConflicts: false,
		});
		buildYamlDryRunDiff({
			currentYaml: structuralCurrentYaml,
			dryRunYaml: structuralDryRunYaml,
			style: "git",
			full: true,
			forceConflicts: false,
		});
	});

	bench("F16 full clean diff with conflict presentation", () => {
		buildYamlDryRunDiff({
			currentYaml,
			dryRunYaml,
			style: "clean",
			full: true,
			forceConflicts: true,
		});
	});

	bench("F16 identical full git diff with newline normalization", () => {
		buildYamlDryRunDiff({
			currentYaml: identicalYaml,
			dryRunYaml: identicalYaml,
			style: "git",
			full: true,
			forceConflicts: false,
		});
	});
});
