import { describe, expect, test } from "bun:test";
import type { RbacSubjectSummary } from "@/lib/types";
import { serviceAccountMatcher, splitServiceAccountUsername, subjectMatcher } from "./principal";

// Failure modes: a ServiceAccount username in a User subject is ignored; a
// malformed or prefix-only username matches an account; a username for another
// namespace or name matches; a Group identity matches a User subject.
const subject = (kind: string, name: string, namespace?: string): RbacSubjectSummary => ({ kind, name, namespace });

describe("RBAC subject matcher", () => {
	test("splits only well-formed ServiceAccount usernames", () => {
		expect(splitServiceAccountUsername("system:serviceaccount:payments:api")).toEqual({ namespace: "payments", name: "api" });
		for (const username of [
			"system:serviceaccount:payments",
			"system:serviceaccount::api",
			"system:serviceaccount:payments:",
			"system:serviceaccount:payments:api:extra",
			"system:serviceaccounts:payments:api",
			"alice",
		]) expect(splitServiceAccountUsername(username)).toBeNull();
	});

	test("a ServiceAccount matches its username and automatic groups only", () => {
		const account = serviceAccountMatcher("payments", "api");
		expect(account(subject("User", "system:serviceaccount:payments:api"))).toBe(true);
		expect(account(subject("ServiceAccount", "api", "payments"))).toBe(true);
		expect(account(subject("Group", "system:serviceaccounts:payments"))).toBe(true);
		expect(account(subject("User", "system:serviceaccount:other:api"))).toBe(false);
		expect(account(subject("User", "system:serviceaccount:payments:web"))).toBe(false);
		expect(account(subject("Group", "system:serviceaccounts:other"))).toBe(false);
	});

	test("a user named as a ServiceAccount matches that account's subject", () => {
		const user = subjectMatcher({ kind: "user", username: "system:serviceaccount:payments:api", groups: [] });
		expect(user(subject("ServiceAccount", "api", "payments"))).toBe(true);
		expect(user(subject("Group", "system:authenticated"))).toBe(false);
	});

	test("a Group identity matches only its group", () => {
		const group = subjectMatcher({ kind: "group", group: "ops" });
		expect(group(subject("Group", "ops"))).toBe(true);
		expect(group(subject("User", "ops"))).toBe(false);
		expect(group(subject("User", ""))).toBe(false);
	});
});
