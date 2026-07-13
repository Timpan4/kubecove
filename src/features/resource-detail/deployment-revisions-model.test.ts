import {
	deploymentRevisionViewState,
	reconcileRevisionSelection,
	revisionLabel,
} from "./deployment-revisions-model";
import type { DeploymentRevision } from "@/lib/types";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

function revision(name: string, number?: number): DeploymentRevision {
	return { name, revision: number, podTemplateYaml: `name: ${name}` };
}

describe("deployment revision view model", () => {
	test("represents loading, error, empty, single, and comparison states", () => {
		expect(deploymentRevisionViewState(true, false, [])).toBe("loading");
		expect(deploymentRevisionViewState(false, true, [])).toBe("error");
		expect(deploymentRevisionViewState(false, false, [])).toBe("empty");
		expect(deploymentRevisionViewState(false, false, [revision("rs-1")])).toBe("single");
		expect(
			deploymentRevisionViewState(false, false, [revision("rs-2"), revision("rs-1")]),
		).toBe("compare");
	});

	test("keeps any valid pair of distinct revision selections", () => {
		const revisions = [revision("rs-3", 3), revision("rs-2", 2), revision("rs-1", 1)];

		expect(reconcileRevisionSelection(revisions, "rs-2", "rs-1")).toEqual({
			selectedName: "rs-2",
			comparisonName: "rs-1",
		});
	});

	test("replaces missing or duplicate selections with distinct available revisions", () => {
		const revisions = [revision("rs-3", 3), revision("rs-2", 2), revision("rs-1", 1)];

		expect(reconcileRevisionSelection(revisions, "missing", "rs-3")).toEqual({
			selectedName: "rs-3",
			comparisonName: "rs-2",
		});
		expect(reconcileRevisionSelection(revisions, "rs-2", "rs-2")).toEqual({
			selectedName: "rs-2",
			comparisonName: "rs-3",
		});
	});

	test("labels numeric revisions and falls back to ReplicaSet names", () => {
		expect(revisionLabel(revision("rs-3", 3))).toBe("Revision 3");
		expect(revisionLabel(revision("rs-unknown"))).toBe("rs-unknown");
	});
});
