import { bench, describe } from "vitest";
import {
	buildDedupedResourceSearchIndex,
	dedupeResources,
} from "@/features/command-palette/entries";
import { buildResourceSearchIndex } from "@/features/resources/helpers";
import type { ResourceSummary } from "@/lib/types";

function resource(index: number): ResourceSummary {
	return {
		cluster: "prod",
		apiVersion: index % 2 === 0 ? "v1" : "apps/v1",
		kind: index % 2 === 0 ? "Pod" : "Deployment",
		name: `resource-${index}`,
		namespace: `namespace-${index % 100}`,
		age: "1m",
	};
}

const warmedRows = Array.from({ length: 5_000 }, (_, index) => resource(index));
const cachedRows = Array.from({ length: 5_000 }, (_, index) =>
	resource(index + 2_500),
);
const resourceSets = [warmedRows, cachedRows];

describe("command palette resource index (10k overlapping rows)", () => {
	bench("merge + dedupe + index", () => {
		buildResourceSearchIndex(dedupeResources(resourceSets.flat()));
	});

	bench("deduped index in one pass", () => {
		buildDedupedResourceSearchIndex(resourceSets);
	});
});
