import type { RbacAccessReviewIdentity, RbacSubjectSummary } from "@/lib/types";

// Kubernetes matches a ServiceAccount subject by its username, so a User
// subject named `system:serviceaccount:<namespace>:<name>` grants that account.
const SERVICE_ACCOUNT_PREFIX = "system:serviceaccount:";

/** The user name and groups a SubjectAccessReview sends for an identity, mapped as `rbac_review.rs` does. */
export interface RbacPrincipal {
	username?: string;
	/** The ServiceAccount the username names, parsed once for subject matching. */
	account: { namespace: string; name: string } | null;
	groups: string[];
}

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

export function principalOf(identity: RbacAccessReviewIdentity): RbacPrincipal {
	if (identity.kind === "serviceAccount") {
		const username = serviceAccountUsername(identity.namespace, identity.name);
		return { username, account: splitServiceAccountUsername(username), groups: automaticServiceAccountGroups(identity.namespace) };
	}
	if (identity.kind === "user") {
		return { username: identity.username, account: splitServiceAccountUsername(identity.username), groups: identity.groups };
	}
	return { account: null, groups: [identity.group] };
}

export function serviceAccountPrincipal(namespace: string, name: string): RbacPrincipal {
	return principalOf({ kind: "serviceAccount", name, namespace });
}

/** `appliesToUser` in the Kubernetes RBAC authorizer. */
export function subjectAppliesTo(principal: RbacPrincipal, subject: RbacSubjectSummary): boolean {
	if (subject.kind === "User") return principal.username !== undefined && subject.name === principal.username;
	if (subject.kind === "ServiceAccount") {
		const account = principal.account;
		return account !== null && subject.namespace === account.namespace && subject.name === account.name;
	}
	return subject.kind === "Group" && principal.groups.includes(subject.name);
}
