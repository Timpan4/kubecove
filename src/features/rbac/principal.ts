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

/** The user name, ServiceAccount and groups the authorizer matches for one identity. */
export interface PrincipalIdentity {
	/** The User subject name, or undefined when the identity has none. */
	username: string | undefined;
	account: { namespace: string; name: string } | null;
	groups: readonly string[];
}

/** Maps an identity the way `rbac_review.rs` does, so every RBAC lookup normalizes identities the same way. */
export function principalOf(identity: RbacAccessReviewIdentity): PrincipalIdentity {
	switch (identity.kind) {
		case "serviceAccount":
			return {
				username: serviceAccountUsername(identity.namespace, identity.name),
				account: { namespace: identity.namespace, name: identity.name },
				groups: automaticServiceAccountGroups(identity.namespace),
			};
		case "user":
			return {
				username: identity.username,
				account: splitServiceAccountUsername(identity.username),
				groups: identity.groups,
			};
		case "group":
			return { username: undefined, account: null, groups: [identity.group] };
	}
}

/**
 * `appliesToUser` in the Kubernetes RBAC authorizer, for the user name and
 * groups a SubjectAccessReview sends for the identity. Build it once per identity: the returned check runs
 * for every subject of every binding.
 */
export function subjectMatcher(identity: RbacAccessReviewIdentity): RbacSubjectMatcher {
	const { username, account, groups } = principalOf(identity);
	const groupSet: ReadonlySet<string> = new Set(groups);
	const accountName = account?.name;
	const accountNamespace = account?.namespace;
	return (subject) => {
		if (subject.kind === "ServiceAccount") return accountName !== undefined && subject.name === accountName && subject.namespace === accountNamespace;
		if (subject.kind === "Group") return groupSet.has(subject.name);
		return subject.kind === "User" && username !== undefined && subject.name === username;
	};
}

export function serviceAccountMatcher(namespace: string, name: string): RbacSubjectMatcher {
	return subjectMatcher({ kind: "serviceAccount", name, namespace });
}
