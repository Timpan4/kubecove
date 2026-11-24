import { createTauriClient, reviewRbacAccess } from "@/lib/tauri";
import type { RbacAccessReviewRequest, RbacAccessReviewResult, RbacInspectionSummary } from "@/lib/types";
import { stepRbacHandoff, type RbacVerifierHandoff } from "./handoff";
import {
	identityDefaults,
	inspectorIdentity,
	missingServiceAccountNamespace,
	reviewIdentity,
	type InspectorIdentity,
} from "./observedPermissions";

interface RbacVerifierOptions {
	data: () => RbacInspectionSummary | undefined;
	selectedIdentity: () => InspectorIdentity | null;
	handoff: () => RbacVerifierHandoff | undefined;
	onHandoffConsumed: () => void;
}

export function rbacVerifier(options: RbacVerifierOptions) {
	const client = createTauriClient();
	const data = $derived(options.data());
	const selectedIdentity = $derived(options.selectedIdentity());
	// Which identity the verifier checks: the selected object, the manual form, or a drilled-in subject.
	type IdentitySource =
		| { kind: "selection" }
		| { kind: "manual" }
		| { kind: "subject"; identity: InspectorIdentity };
	type SubmittedReview = { timestamp: string; identity: string; target: string };
	type AccessReview =
		| { kind: "none" }
		| { kind: "checking"; submitted: SubmittedReview }
		| { kind: "done"; submitted: SubmittedReview; result: RbacAccessReviewResult }
		| { kind: "failed"; submitted: SubmittedReview; message: string };
	let identitySource = $state<IdentitySource>({ kind: "selection" });
	let handoffPromoted = $state(false);
	// Raw state keeps object identity, which verify() compares to drop stale results.
	let accessReview = $state.raw<AccessReview>({ kind: "none" });
	const reviewing = $derived(accessReview.kind === "checking");
	let verb = $state("get");
	let resource = $state("pods");
	let apiGroup = $state("");
	let subresource = $state("");
	let resourceName = $state("");
	let namespace = $state("");
	let targetKind = $state<"resource" | "nonResource">("resource");
	let nonResourceUrl = $state("/healthz");
	let handoffKey = $state("");
	let identityKind = $state<"serviceAccount" | "user" | "group">("user");
	let identityName = $state("");
	let identityNamespace = $state("default");
	let identityGroups = $state("system:authenticated");

	$effect(() => {
		const handoff = options.handoff();
		const step = stepRbacHandoff(handoffKey, handoff);
		handoffKey = step.key;
		if (!handoff || !step.apply) return;
		handoffPromoted = true;
		const handoffIdentity = inspectorIdentity(handoff.identity);
		identitySource = handoffIdentity ? { kind: "subject", identity: handoffIdentity } : { kind: "manual" };
		if (handoff.target.kind === "resource") {
			targetKind = "resource";
			verb = handoff.target.verb;
			resource = handoff.target.resource;
			apiGroup = handoff.target.apiGroup ?? "";
			namespace = handoff.target.namespace ?? "";
			subresource = handoff.target.subresource ?? "";
			resourceName = handoff.target.name ?? "";
		} else {
			targetKind = "nonResource";
			verb = handoff.target.verb;
			nonResourceUrl = handoff.target.nonResourceUrl;
		}
		accessReview = { kind: "none" };
		options.onHandoffConsumed();
	});
	const activeIdentity: InspectorIdentity = $derived.by(() => {
		if (identitySource.kind === "subject") return identitySource.identity;
		if (identitySource.kind === "selection" && selectedIdentity) return selectedIdentity;
		return manualIdentityValue();
	});
	const canVerify = $derived.by(() => {
		if (!data || !activeIdentity.name || !verb.trim()) return false;
		if (missingServiceAccountNamespace(activeIdentity)) return false;
		if (targetKind === "resource") return resource.trim() !== "";
		return nonResourceUrl.trim().startsWith("/");
	});
	const fingerprint = $derived(
		JSON.stringify({
			cluster: data?.cluster,
			refreshedAt: data?.refreshedAt,
			activeIdentity,
			targetKind,
			verb,
			apiGroup,
			resource,
			namespace,
			subresource,
			resourceName,
			nonResourceUrl,
		}),
	);

	$effect(() => {
		void fingerprint;
		accessReview = { kind: "none" };
	});

	function manualIdentityValue(): InspectorIdentity {
		if (identityKind === "serviceAccount") {
			return {
				kind: "serviceAccount",
				name: identityName.trim(),
				namespace: identityNamespace.trim(),
			};
		}
		if (identityKind === "group") {
			return { kind: "group", name: identityName.trim() };
		}
		return {
			kind: "user",
			name: identityName.trim(),
			groups: uniqueGroups(identityGroups),
		};
	}

	function beginIdentity(kind: "serviceAccount" | "user" | "group" = "user") {
		identitySource = { kind: "manual" };
		identityKind = kind;
		identityName = "";
		identityNamespace = "default";
		identityGroups = identityDefaults(kind).join(", ");
	}

	// An identity typed while the selection falls back to the form makes it manual, so the form stays editable.
	function editManualIdentity() {
		identitySource = { kind: "manual" };
	}

	// Subjects of any other kind are users, matching how the RBAC inspector lists them.
	function subjectIdentityKind(kind: string): InspectorIdentity["kind"] {
		if (kind === "ServiceAccount") return "serviceAccount";
		if (kind === "Group") return "group";
		return "user";
	}

	function inspectSubject(subject: {
		kind: string;
		name: string;
		namespace?: string;
	}) {
		identitySource = {
			kind: "subject",
			identity: {
				kind: subjectIdentityKind(subject.kind),
				name: subject.name,
				namespace: subject.namespace,
			},
		};
	}

	async function verify() {
		if (!data || !canVerify) return;
		const identity = reviewIdentity(activeIdentity);
		const target =
			targetKind === "resource"
				? {
						kind: "resource" as const,
						verb: verb.trim(),
						resource: resource.trim(),
						apiGroup: apiGroup.trim(),
						namespace: namespace.trim() || null,
						subresource: subresource.trim() || undefined,
						name: resourceName.trim() || undefined,
					}
				: {
						kind: "nonResource" as const,
						verb: verb.trim(),
						nonResourceUrl: nonResourceUrl.trim(),
					};
		const request: RbacAccessReviewRequest = {
			clusterContext: data.cluster,
			identity,
			target,
			requestId: "rbac-review",
			cancelScope: "rbac-review",
		};
		const submitted: SubmittedReview = {
			timestamp: new Date().toISOString(),
			identity: JSON.stringify(identity),
			target: JSON.stringify(target),
		};
		// Any input change resets accessReview, so a result for an older request is dropped here.
		const checking: AccessReview = { kind: "checking", submitted };
		accessReview = checking;
		try {
			const result = await reviewRbacAccess(client, request);
			if (accessReview === checking) accessReview = { kind: "done", submitted, result };
		} catch (error) {
			if (accessReview === checking) {
				accessReview = {
					kind: "failed",
					submitted,
					message: error instanceof Error ? error.message : String(error),
				};
			}
		}
	}

	function uniqueGroups(value: string): string[] {
		return [
			...new Set(
				value
					.split(",")
					.map((group) => group.trim())
					.filter(Boolean),
			),
		];
	}

	function useSelection(clearHandoff = false) {
		identitySource = { kind: "selection" };
		if (clearHandoff) handoffPromoted = false;
	}

	return {
		get activeIdentity() { return activeIdentity; },
		get manual() { return identitySource.kind === "manual"; },
		get handoffPromoted() { return handoffPromoted; },
		get accessReview() { return accessReview; },
		get reviewing() { return reviewing; },
		get canVerify() { return canVerify; },
		get verb() { return verb; },
		set verb(value: typeof verb) { verb = value; },
		get resource() { return resource; },
		set resource(value: typeof resource) { resource = value; },
		get apiGroup() { return apiGroup; },
		set apiGroup(value: typeof apiGroup) { apiGroup = value; },
		get subresource() { return subresource; },
		set subresource(value: typeof subresource) { subresource = value; },
		get resourceName() { return resourceName; },
		set resourceName(value: typeof resourceName) { resourceName = value; },
		get namespace() { return namespace; },
		set namespace(value: typeof namespace) { namespace = value; },
		get targetKind() { return targetKind; },
		set targetKind(value: typeof targetKind) { targetKind = value; },
		get nonResourceUrl() { return nonResourceUrl; },
		set nonResourceUrl(value: typeof nonResourceUrl) { nonResourceUrl = value; },
		get identityKind() { return identityKind; },
		set identityKind(value: typeof identityKind) { identityKind = value; },
		get identityName() { return identityName; },
		set identityName(value: typeof identityName) { identityName = value; },
		get identityNamespace() { return identityNamespace; },
		set identityNamespace(value: typeof identityNamespace) { identityNamespace = value; },
		get identityGroups() { return identityGroups; },
		set identityGroups(value: typeof identityGroups) { identityGroups = value; },
		beginIdentity, editManualIdentity, inspectSubject, useSelection, verify,
	};
}

export function identityLabel(identity: InspectorIdentity): string {
		if (!identity.name) return "No identity selected";
		if (identity.kind === "serviceAccount") {
			return `system:serviceaccount:${identity.namespace ?? "default"}:${identity.name}`;
		}
		return identity.kind === "group"
			? `Group/${identity.name}`
			: `User/${identity.name}`;
	}
