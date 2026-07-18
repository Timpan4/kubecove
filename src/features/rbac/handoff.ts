import type { RbacAccessReviewIdentity, RbacAccessReviewTarget } from "@/lib/types";
import type { ResourceSummary } from "@/lib/types";

export const OPEN_RBAC_VERIFIER_EVENT = "kubecove:open-rbac-verifier";

export interface RbacVerifierHandoff {
	target: RbacAccessReviewTarget;
	identity?: RbacAccessReviewIdentity;
	sourceLabel?: string;
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

export function openRbacVerifier(handoff: RbacVerifierHandoff): void {
	if (typeof window === "undefined") return;
	window.dispatchEvent(new CustomEvent<RbacVerifierHandoff>(OPEN_RBAC_VERIFIER_EVENT, { detail: handoff }));
}

export function onOpenRbacVerifier(listener: (handoff: RbacVerifierHandoff) => void): () => void {
	if (typeof window === "undefined") return () => {};
	const handle = (event: Event) => { if (event instanceof CustomEvent) listener(event.detail as RbacVerifierHandoff); };
	window.addEventListener(OPEN_RBAC_VERIFIER_EVENT, handle);
	return () => window.removeEventListener(OPEN_RBAC_VERIFIER_EVENT, handle);
}
