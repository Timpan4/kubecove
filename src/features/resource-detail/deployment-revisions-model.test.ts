import { reconcileRevisionSelection } from "./deployment-revisions-model";
import type { DeploymentRevision } from "@/lib/types";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe<Expected>(expected: Expected): void;
	toEqual<Expected>(expected: Expected): void;
};

function revision(name: string, number?: number): DeploymentRevision {
	return { name, revision: number, podTemplateYaml: `name: ${name}` };
}

describe("deployment revision view model", () => {
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
});
