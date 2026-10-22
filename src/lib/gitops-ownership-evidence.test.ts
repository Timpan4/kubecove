import { expect, test } from "bun:test";
import { inheritGitOpsOwnership } from "./gitops-ownership-evidence";
import type { GitOpsOwnerSummary, ResourceSummary } from "./types";

// Failure modes: unchanged inherited rows lose identity on every projection,
// changed ownership stays cached, or removed ownership leaks from a prior scope.
test("inherited rows retain identity until their owner value changes", () => {
	const owner: GitOpsOwnerSummary = { provider: "argo", kind: "Application", name: "app", confidence: "metadata" };
	const parent: ResourceSummary = {
		cluster: "dev", kind: "Deployment", name: "api", namespace: "ns", age: "", health: "healthy",
		gitOpsOwner: owner,
	};
	const child: ResourceSummary = { ...parent, kind: "Pod", name: "api-0", ownerRef: "api", gitOpsOwner: undefined };
	const first = inheritGitOpsOwnership([parent, child])[1];
	expect(first).not.toBe(child);
	expect(inheritGitOpsOwnership([parent, child])[1]).toBe(first);
	const sameOwner = { ...parent, gitOpsOwner: { ...owner } };
	expect(inheritGitOpsOwnership([sameOwner, child])[1]).toBe(first);
	const changedOwner = { ...parent, gitOpsOwner: { ...owner, name: "other" } };
	const changed = inheritGitOpsOwnership([changedOwner, child])[1];
	expect(changed).not.toBe(first);
	expect(changed?.gitOpsOwner?.name).toBe("other");
	expect(inheritGitOpsOwnership([child])[0]).toBe(child);
	const unownedParent = { ...parent, gitOpsOwner: undefined };
	expect(inheritGitOpsOwnership([unownedParent, child])[1]).toBe(child);
});
