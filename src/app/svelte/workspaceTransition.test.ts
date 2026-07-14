import {
	cancelWorkspaceWork,
	createWorkspaceTransitionCoordinator,
	type WorkspaceTransitionCoordinator,
} from "./workspaceTransition";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void | Promise<void>): void;
declare function expect<T>(actual: T): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toHaveLength(expected: number): void;
};

function deferred() {
	let resolve!: () => void;
	const promise = new Promise<void>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

describe("workspace transition coordinator", () => {
	test("starts backend cancellation without waiting for query cancellation", async () => {
		const queries = deferred();
		const backend = deferred();
		const events: string[] = [];

		const cancellation = cancelWorkspaceWork(
			async () => {
				events.push("queries:start");
				await queries.promise;
				events.push("queries:done");
			},
			async () => {
				events.push("backend:start");
				await backend.promise;
				events.push("backend:done");
			},
		);

		expect(events).toEqual(["queries:start", "backend:start"]);
		backend.resolve();
		await cancellation;
		expect(events).toEqual([
			"queries:start",
			"backend:start",
			"backend:done",
		]);
		queries.resolve();
		await Promise.resolve();
		expect(events).toEqual([
			"queries:start",
			"backend:start",
			"backend:done",
			"queries:done",
		]);
	});

	test("applies selection only after cancellation finishes", async () => {
		const cancellation = deferred();
		const events: string[] = [];
		const coordinator = createWorkspaceTransitionCoordinator<string>({
			suspend: () => {
				events.push("suspend");
			},
			cancel: async () => {
				events.push("cancel");
				await cancellation.promise;
			},
			apply: (destination) => {
				events.push(`apply:${destination}`);
			},
			resume: () => {
				events.push("resume");
			},
		});

		const transition = coordinator.request("healthy");
		await Promise.resolve();
		expect(events).toEqual(["suspend", "cancel"]);
		cancellation.resolve();
		await transition;
		expect(events).toEqual(["suspend", "cancel", "apply:healthy", "resume"]);
	});

	test("coalesces rapid transitions to the latest destination", async () => {
		const cancellation = deferred();
		const applied: string[] = [];
		let cancellations = 0;
		const coordinator = createWorkspaceTransitionCoordinator<string>({
			suspend: () => {},
			cancel: async () => {
				cancellations += 1;
				await cancellation.promise;
			},
			apply: (destination) => applied.push(destination),
			resume: () => {},
		});

		const first = coordinator.request("B");
		const second = coordinator.request("C");
		cancellation.resolve();
		await Promise.all([first, second]);

		expect(cancellations).toBe(1);
		expect(applied).toEqual(["C"]);
	});

	test("drains a destination requested by a resume microtask", async () => {
		const applied: string[] = [];
		let coordinator!: WorkspaceTransitionCoordinator<string>;
		let queueNext = true;
		coordinator = createWorkspaceTransitionCoordinator<string>({
			suspend: () => {},
			cancel: async () => {},
			apply: (destination) => applied.push(destination),
			resume: () => {
				if (!queueNext) return;
				queueNext = false;
				queueMicrotask(() => {
					void coordinator.request("C");
				});
			},
		});

		await coordinator.request("B");

		expect(applied).toEqual(["B", "C"]);
		expect(coordinator.isPending()).toBe(false);
	});

	test("continues the switch when backend cancellation fails", async () => {
		const errors: unknown[] = [];
		const applied: string[] = [];
		let resumed = false;
		const coordinator = createWorkspaceTransitionCoordinator<string>({
			suspend: () => {},
			cancel: async () => {
				throw new Error("backend unavailable");
			},
			apply: (destination) => applied.push(destination),
			resume: () => {
				resumed = true;
			},
			onCancelError: (error) => errors.push(error),
		});

		await coordinator.request("healthy");

		expect(applied).toEqual(["healthy"]);
		expect(errors).toHaveLength(1);
		expect(resumed).toBe(true);
		expect(coordinator.isPending()).toBe(false);
	});
});
