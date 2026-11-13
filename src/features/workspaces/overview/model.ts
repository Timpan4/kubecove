import { resourceKey } from "@/lib/resource-identity";
import type { ResourceSummary } from "@/lib/types";
import type { HealthAssessmentState } from "@/lib/health-types";

export type Tone = HealthAssessmentState;
export type ProblemTone = "degraded" | "needsAttention";
export interface ProblemMember { resource: ResourceSummary; tone: ProblemTone }
export interface Problem {
	key: string;
	resource: ResourceSummary;
	tone: ProblemTone;
	cause: ResourceSummary;
	related: ProblemMember[];
	restarts: number;
}
export interface NamespaceRow {
	key: string;
	namespace: string;
	total: number;
	degraded: number;
	attention: number;
}
export interface OverviewModel { problems: Problem[]; problemResources: number; namespaces: NamespaceRow[] }

export const toneOf = (row: ResourceSummary): Tone => row.healthAssessment?.state ?? "unknown";
function problemTone(row: ResourceSummary): ProblemTone | null {
	const state = toneOf(row);
	return state === "degraded" || state === "needsAttention" ? state : null;
}
const ownerKey = (row: ResourceSummary, uid: string) => JSON.stringify([row.cluster, row.namespace, uid]);

export function buildOverviewModel(rows: readonly ResourceSummary[]): OverviewModel {
	// Only the Kubernetes controller reference and the loaded object's UID establish ownership.
	const owners = new Map<string, ResourceSummary | null>();
	for (const row of rows) {
		if (!row.uid) continue;
		const key = ownerKey(row, row.uid);
		owners.set(key, owners.has(key) ? null : row);
	}
	const groups = new Map<ResourceSummary, ProblemMember[]>();
	const namespaces = new Map<string, NamespaceRow>();
	let problemResources = 0;
	for (const row of rows) {
		const tone = problemTone(row);
		if (row.namespace !== null) {
			let namespace = namespaces.get(row.namespace);
			if (!namespace) {
				namespace = { key: row.namespace, namespace: row.namespace, total: 0, degraded: 0, attention: 0 };
				namespaces.set(row.namespace, namespace);
			}
			namespace.total++;
			if (tone === "degraded") namespace.degraded++;
			if (tone === "needsAttention") namespace.attention++;
		}
		if (!tone) continue;
		problemResources++;
		let root = row;
		let current = row;
		const seen = new Set<ResourceSummary>([row]);
		while (current.controllerOwner) {
			const reference = current.controllerOwner;
			const owner = owners.get(ownerKey(current, reference.uid));
			if (!owner || owner.name !== reference.name || owner.kind !== reference.kind || owner.apiVersion !== reference.apiVersion) break;
			if (seen.has(owner)) { root = row; break; }
			seen.add(owner);
			if (problemTone(owner)) root = owner;
			current = owner;
		}
		const members = groups.get(root);
		if (members) members.push({ resource: row, tone });
		else groups.set(root, [{ resource: row, tone }]);
	}
	const problems: Problem[] = [];
	for (const [resource, members] of groups) {
		const tone = members.some(member => member.tone === "degraded") ? "degraded" : "needsAttention";
		let cause = members.find(member => member.resource === resource)?.resource ?? resource;
		for (const member of members) {
			if (member.tone !== tone) continue;
			if (problemTone(cause) !== tone ||
				(member.resource.kind === "Pod" && (cause.kind !== "Pod" || (member.resource.restarts ?? 0) > (cause.restarts ?? 0)))) cause = member.resource;
		}
		problems.push({ key: resourceKey(resource), resource, tone, cause,
			related: members.filter(member => member.resource !== resource),
			restarts: cause.restarts ?? 0 });
	}
	problems.sort((a, b) => Number(b.tone === "degraded") - Number(a.tone === "degraded") ||
		(a.resource.namespace ?? "").localeCompare(b.resource.namespace ?? "") ||
		a.resource.name.localeCompare(b.resource.name) || a.resource.kind.localeCompare(b.resource.kind) ||
		a.resource.cluster.localeCompare(b.resource.cluster) || (a.resource.apiVersion ?? "").localeCompare(b.resource.apiVersion ?? ""));
	return { problems, problemResources, namespaces: Array.from(namespaces.values()).sort((a, b) =>
		b.degraded - a.degraded || b.attention - a.attention || a.namespace.localeCompare(b.namespace)) };
}
