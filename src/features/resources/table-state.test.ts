import { createTable, getCoreRowModel } from "@tanstack/react-table";
import type { ResourceSummary } from "@/lib/types";
import { columns } from "./columns";
import { createResourceTableState } from "./table-state";

declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
};

const pod = {
	kind: "Pod",
	cluster: "test-cluster",
	name: "api-7c9f",
	namespace: "default",
	age: "5m",
	health: "healthy",
	status: "Running",
	ready: "true",
	restarts: 0,
} satisfies ResourceSummary;

test("resource table controlled state includes TanStack feature defaults", () => {
	const table = createTable<ResourceSummary>({
		data: [pod],
		columns,
		state: createResourceTableState([], {}),
		onStateChange: () => {},
		getCoreRowModel: getCoreRowModel(),
		renderFallbackValue: null,
	});

	expect(table.getHeaderGroups().length).toBe(1);
	expect(table.getCenterTotalSize() > 0).toBe(true);
});
