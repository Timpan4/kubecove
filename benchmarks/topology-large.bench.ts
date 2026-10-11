import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	buildFlowTopologyLayoutState,
	buildFlowTopologyViewFromLayoutState,
} from "@/features/resources/topology-view";
import { buildOwnershipTopology } from "./topology-benchmark-fixtures";

const topology = buildOwnershipTopology(2_000);
const selectedIds = topology.nodes
	.filter((node) => node.kind === "Pod")
	.slice(0, 20)
	.map((node) => node.id);
const expandedStandaloneKinds = new Set<string>();
const viewportSize = { width: 1440, height: 900 };
const layoutState = buildFlowTopologyLayoutState(
	topology,
	"ownership",
	expandedStandaloneKinds,
	null,
);

assert.equal(topology.nodes.length, 4_000);
assert.equal(selectedIds.length, 20);

describe("F07 large ownership layout (2k apps / 4k nodes)", () => {
	bench("F07 layout state construction including selection index", () => {
		buildFlowTopologyLayoutState(
			topology,
			"ownership",
			expandedStandaloneKinds,
			null,
		);
	});

	bench("F09 retained selection and viewport extent (20 updates)", () => {
		for (const [index, selectedNodeId] of selectedIds.entries()) {
			buildFlowTopologyViewFromLayoutState(layoutState, {
				mode: "ownership",
				selectedNodeId,
				showFullTopologyOnSelection: index % 2 === 0,
				expandedStandaloneKinds,
				viewportSize,
			});
		}
	});
});
