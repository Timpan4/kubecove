import type { RbacAccessReviewIdentity, RbacSubjectSummary } from "@/lib/types";

// Kubernetes matches a ServiceAccount subject by its username, so a User
// subject named `system:serviceaccount:<namespace>:<name>` grants that account.
const SERVICE_ACCOUNT_PREFIX = "system:serviceaccount:";

export type RbacSubjectMatcher = (subject: RbacSubjectSummary) => boolean;

export function automaticServiceAccountGroups(namespace: string): string[] {
	return ["system:serviceaccounts", `system:serviceaccounts:${namespace}`, "system:authenticated"];
}

/** `serviceaccount.MakeUsername` in Kubernetes. */
export function serviceAccountUsername(namespace: string, name: string): string {
	return `${SERVICE_ACCOUNT_PREFIX}${namespace}:${name}`;
}

/** `serviceaccount.SplitUsername`: the account a username names, or null for any other username. */
export function splitServiceAccountUsername(username: string): { namespace: string; name: string } | null {
	if (!username.startsWith(SERVICE_ACCOUNT_PREFIX)) return null;
	const parts = username.slice(SERVICE_ACCOUNT_PREFIX.length).split(":");
	if (parts.length !== 2) return null;
	const [namespace, name] = parts;
	return namespace && name ? { namespace, name } : null;
}

/**
 * `appliesToUser` in the Kubernetes RBAC authorizer, for the user name and
 * groups a SubjectAccessReview sends for the identity, mapped as
 * `rbac_review.rs` does. Build it once per identity: the returned check runs
 * for every subject of every binding.
 */
export function subjectMatcher(identity: RbacAccessReviewIdentity): RbacSubjectMatcher {
	let username: string | undefined;
	let account: { namespace: string; name: string } | null;
	let groups: ReadonlySet<string>;
	if (identity.kind === "serviceAccount") {
		username = serviceAccountUsername(identity.namespace, identity.name);
		account = { namespace: identity.namespace, name: identity.name };
		groups = new Set(automaticServiceAccountGroups(identity.namespace));
	} else if (identity.kind === "user") {
		username = identity.username;
		account = splitServiceAccountUsername(identity.username);
		groups = new Set(identity.groups);
	} else {
		account = null;
		groups = new Set([identity.group]);
	}
	const accountName = account?.name;
	const accountNamespace = account?.namespace;
	return (subject) => {
		if (subject.kind === "ServiceAccount") return accountName !== undefined && subject.name === accountName && subject.namespace === accountNamespace;
		if (subject.kind === "Group") return groups.has(subject.name);
		return subject.kind === "User" && username !== undefined && subject.name === username;
	};
}

export function serviceAccountMatcher(namespace: string, name: string): RbacSubjectMatcher {
	return subjectMatcher({ kind: "serviceAccount", name, namespace });
}
