import { expect, test } from "bun:test";
import { projectArgoResourceComparison } from "../src/features/gitops/argo-resource-display";

test("Argo rows serialize only requested YAML and reuse identical live and normalized text", () => {
	const reads: string[] = [];
	const manifest = (name: string) => ({
		get kind() {
			reads.push(name);
			return "ConfigMap";
		},
	});
	const removal = { requiresPruning: true, liveState: manifest("pruned") };
	const resource = { targetState: manifest("desired"), liveState: manifest("live") };
	const pruned = projectArgoResourceComparison(removal, null);
	const documents = projectArgoResourceComparison(resource, {
		resource,
		modified: true,
		availableActions: [],
	});

	expect(documents.added).toBe(false);
	expect(documents.modified).toBe(true);
	expect(documents.hasDiff).toBe(true);
	expect(reads).toEqual([]);

	expect(documents.desired).toBe("kind: ConfigMap");
	expect(documents.desired).toBe("kind: ConfigMap");
	expect(reads).toEqual(["desired"]);

	expect(documents.normalizedLive).toBe("kind: ConfigMap");
	expect(documents.live).toBe("kind: ConfigMap");
	expect(pruned.live).toBe("kind: ConfigMap");
	expect(reads).toEqual(["desired", "live", "pruned"]);
});

test("Argo badge fallback compares displayed text when the server omits modified", () => {
	const documents = projectArgoResourceComparison({
		targetState: "kind: ConfigMap\n",
		liveState: { kind: "ConfigMap" },
	}, null);
	expect(documents.modified).toBe(false);

	const added = projectArgoResourceComparison({
		targetState: { kind: "ConfigMap" },
		liveState: "\n ",
	}, null);
	expect(added.added).toBe(true);
	expect(added.modified).toBe(true);
});
