import { describe, expect, test } from "bun:test";
import { resourceLifecycle } from "./resource-lifecycle";

// Failure modes: unknown or terminal states spin; warnings and failures look like
// normal progress; a non-workload Pending state is mistaken for a pod starting.
describe("resource lifecycle", () => {
	test.each([
		["Deployment", "Rolling out", "Rolling out"],
		["Pod", "Starting", "Starting"],
		["Pod", "Pending", "Starting"],
		["Pod", "Terminating", "Terminating"],
		["ReplicaSet", "Starting", "Starting"],
		["ReplicaSet", "Scaling down", "Scaling down"],
	])("%s %s is light blue progress", (kind, status, label) => {
		expect(resourceLifecycle(kind, status)).toEqual({ label, progress: true, tone: "info" });
	});
	test.each(["ImagePullBackOff", "CrashLoopBackOff", "Rollout failed", "ErrImagePull"])("%s is a static failure", (status) => {
		expect(resourceLifecycle("Pod", status)).toEqual({ label: status, progress: false, tone: "error" });
	});
	test("paused rollouts are warnings", () => {
		expect(resourceLifecycle("Deployment", "Paused")).toEqual({ label: "Paused", progress: false, tone: "warning" });
	});
	test.each(["Running", "Succeeded", "Failed", "Unknown", "Available: 1", undefined])("%s does not invent ongoing activity", (status) => {
		expect(resourceLifecycle("Pod", status)?.progress ?? false).toBe(false);
	});
	test("Pending claims are not pod startup", () => {
		expect(resourceLifecycle("PersistentVolumeClaim", "Pending")).toBeNull();
	});
});
