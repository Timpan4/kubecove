import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import { parse } from "yaml";
import { formatYamlDocument } from "@/lib/yamlFormat";
import { buildYamlManifest } from "./yaml-benchmark-fixtures";

const validYaml = buildYamlManifest(40);
const styledYaml = `# keep semantic values across formatter styles
apiVersion: v1
kind: ConfigMap
metadata:
  name: "checkout-api"
  labels:
    quoted-boolean: 'true'
    hash-text: "literal # marker"
data:
  folded: >-
    preserve this
    as one scalar
  literal: |
    first line
    second line
  flow-values: [alpha, "true", 42]
  nested-values:
    - key: 'value: # literal'
`;
const duplicateKeyYaml = `${validYaml}\n  replicas: 8`;
const yamlOutput = formatYamlDocument(validYaml, "yaml");
const kyamlOutput = formatYamlDocument(validYaml, "kyaml");
const styledYamlOutput = formatYamlDocument(styledYaml, "yaml");
const styledKyamlOutput = formatYamlDocument(styledYaml, "kyaml");
let rejectedDuplicateKey = false;
try {
	formatYamlDocument(duplicateKeyYaml, "yaml");
} catch {
	rejectedDuplicateKey = true;
}

assert(validYaml.includes("worker-39"));
assert(yamlOutput.includes("worker-39"));
assert(kyamlOutput.includes("worker-39"));
assert.deepEqual(parse(styledYamlOutput), parse(styledYaml));
assert.deepEqual(parse(styledKyamlOutput), parse(styledYaml));
assert(rejectedDuplicateKey);

describe("F19 explicit YAML and KYAML formatting (40 containers)", () => {
	bench("F19 validate and format full document as YAML", () => {
		formatYamlDocument(validYaml, "yaml");
		formatYamlDocument(styledYaml, "yaml");
	});

	bench("F19 validate and format full document as KYAML", () => {
		formatYamlDocument(validYaml, "kyaml");
		formatYamlDocument(styledYaml, "kyaml");
	});

	bench("F19 reject duplicate keys through production formatter", () => {
		try {
			formatYamlDocument(duplicateKeyYaml, "yaml");
		} catch {
			// The explicit formatter call remains the timed failure path.
		}
	});
});
