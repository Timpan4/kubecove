import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	decodeSecretDataValue,
	parseSecretData,
} from "@/features/resource-detail/secretData";

const decodedSecretBytes = 1_048_576;
const decodedSecretValue = "x".repeat(decodedSecretBytes);
const selectedEncodedValue = btoa(decodedSecretValue);
const binaryEncodedValue = btoa(String.fromCharCode(0xff, 0xfe, 0xfd));
const secretYaml = [
	"apiVersion: v1",
	"kind: Secret",
	"metadata:",
	"  name: synthetic-secret",
	"  namespace: checkout",
	"type: Opaque",
	"data:",
	`  selected: "${selectedEncodedValue}"`,
	'  invalid: "%%%"',
	`  binary: "${binaryEncodedValue}"`,
	"stringData:",
	'  redacted: "[redacted]"',
].join("\n");
const entries = parseSecretData(secretYaml);
const selected = entries.find((entry) => entry.key === "selected");
const decoded = selected ? decodeSecretDataValue(selected.value) : null;

assert.equal(entries.length, 4);
assert.equal(selected?.value.length, Math.ceil(decodedSecretBytes / 3) * 4);
assert.equal(decoded?.value?.length, decodedSecretBytes);
assert(entries.find((entry) => entry.key === "redacted")?.masked);
assert.deepEqual(decodeSecretDataValue("%%%"), { error: "invalid-base64" });
assert.deepEqual(decodeSecretDataValue(binaryEncodedValue), {
	error: "binary-data",
});

describe("F18 Secret YAML parsing and selected-value decoding", () => {
	bench("F18 parse a 1 MiB synthetic Secret and project entries", () => {
		parseSecretData(secretYaml);
	});

	bench(
		"F18 decode the selected 1 MiB value and validate sibling failures",
		() => {
			decodeSecretDataValue(selectedEncodedValue);
			decodeSecretDataValue("%%%");
			decodeSecretDataValue(binaryEncodedValue);
		},
	);
});
