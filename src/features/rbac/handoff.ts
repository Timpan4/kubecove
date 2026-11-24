import type { RbacAccessReviewIdentity, RbacAccessReviewTarget } from "@/lib/types";
import type { ResourceSummary } from "@/lib/types";

export const OPEN_RBAC_VERIFIER_EVENT = "kubecove:open-rbac-verifier";

export interface RbacVerifierHandoff {
	target: RbacAccessReviewTarget;
	identity?: RbacAccessReviewIdentity;
	sourceLabel?: string;
}

declare global {
	interface WindowEventMap {
		"kubecove:open-rbac-verifier": CustomEvent<RbacVerifierHandoff>;
	}
}

export function requiredPermissionForResource(resource: ResourceSummary, verb: string): RbacAccessReviewTarget | null {
	const plural = resource.plural?.trim();
	const operation = verb.trim();
	if (!plural || !operation) return null;
	const namespace = resource.namespace?.trim() || null;
	if (resource.namespaced === true && namespace === null) return null;
	const apiGroup = resource.group?.trim() ?? (resource.apiVersion?.includes("/") ? resource.apiVersion.split("/")[0] ?? "" : "");
	return { kind: "resource", verb: operation, apiGroup, resource: plural, namespace: resource.namespaced === false ? null : namespace, name: resource.name };
}

export interface RbacHandoffStep {
	/** Key of the last handoff applied. Clearing the handoff resets it to "". */
	key: string;
	apply: boolean;
}

/**
 * Decides whether a handoff populates the verifier form. The same action applies again
 * only after the pending handoff clears, so a repeated click repopulates an edited form.
 */
export function stepRbacHandoff(appliedKey: string, handoff: RbacVerifierHandoff | undefined): RbacHandoffStep {
	if (!handoff) return { key: "", apply: false };
	const key = JSON.stringify(handoff);
	return { key, apply: key !== appliedKey };
}

export function openRbacVerifier(handoff: RbacVerifierHandoff): void {
	if (globalThis.window === undefined) return;
	window.dispatchEvent(new CustomEvent<RbacVerifierHandoff>(OPEN_RBAC_VERIFIER_EVENT, { detail: handoff }));
}

export function onOpenRbacVerifier(listener: (handoff: RbacVerifierHandoff) => void): () => void {
	if (globalThis.window === undefined) return () => {};
	const handle = (event: CustomEvent<RbacVerifierHandoff>) => listener(event.detail);
	window.addEventListener(OPEN_RBAC_VERIFIER_EVENT, handle);
	return () => window.removeEventListener(OPEN_RBAC_VERIFIER_EVENT, handle);
}
