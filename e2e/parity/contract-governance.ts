import type { JsonObject } from "../../src/lib/types";
import { type StepParsers, boolean, isString, oneOf, string, noPayload } from "./contract-values";

const RBAC_CATEGORIES = ["Namespace Access", "Roles", "Cluster Roles", "Bindings", "Service Accounts"] as const;
const RBAC_RISKS = ["All", "High", "Medium", "Low", "No flags", "Unknown"] as const;
const VERDICTS = ["allowed", "denied", "noOpinion"] as const;

function reviewNote(item: JsonObject) {
	if (!isString(item.value)) throw new Error("Expected review note text");
	return { value: item.value };
}

export const governanceSteps = {
	filterIncidents: (item) => ({ filter: string(item.filter) }),
	expectIncidentGuide: (item) => ({ title: string(item.title) }),
	openIncidentResources: noPayload,
	selectRbacCategory: (item) => ({ category: oneOf(RBAC_CATEGORIES, item.category, "an RBAC category") }),
	expectRbacCategory: (item) => ({ category: oneOf(RBAC_CATEGORIES, item.category, "an RBAC category") }),
	filterRbacRisk: (item) => ({ filter: oneOf(RBAC_RISKS, item.filter, "an RBAC risk filter") }),
	expectRbacItem: (item) => ({ title: string(item.title), risk: string(item.risk) }),
	expectNoRbacItem: (item) => ({ title: string(item.title) }),
	selectRbacItem: (item) => ({ title: string(item.title) }),
	investigateRbacSubject: (item) => ({ subject: string(item.subject) }),
	expectRbacIdentity: (item) => ({ identity: string(item.identity) }),
	expectRbacGrant: (item) => ({ grant: string(item.grant) }),
	expectNoRbacGrant: (item) => ({ grant: string(item.grant) }),
	verifyRbacAccess: (item) => ({ verb: string(item.verb) }),
	expectRbacVerifyEnabled: (item) => ({ enabled: boolean(item.enabled) }),
	expectRbacVerdict: (item) => ({ outcome: oneOf(VERDICTS, item.outcome, "an access review outcome") }),
	editRbacReviewNote: reviewNote,
	expectRbacReviewNote: reviewNote,
	saveRbacReview: (item) => ({ disposition: oneOf(["expected", "anomalous"], item.disposition, "a review disposition") }),
	expectRbacReview: (item) => ({ label: string(item.label) }),
	clearRbacReview: noPayload,
} satisfies StepParsers;
