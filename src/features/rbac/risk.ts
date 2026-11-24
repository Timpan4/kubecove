import type {
	RbacBindingSummary,
	RbacInspectionSummary,
	RbacRiskIndicator,
	RbacRiskLevel,
	RbacRoleSummary,
	ServiceAccountSummary,
} from "@/lib/types";

const LEVEL_WEIGHT = {
	low: 1,
	medium: 2,
	high: 3,
	unknown: 0,
} satisfies Record<RbacRiskLevel, number>;

export function highestRisk(
	risks: RbacRiskIndicator[],
): RbacRiskIndicator | null {
	return [...risks].sort(
		(a, b) => LEVEL_WEIGHT[b.level] - LEVEL_WEIGHT[a.level],
	)[0] ?? null;
}

export function riskSummaryLabel(risks: RbacRiskIndicator[]): string {
	if (risks.length === 0) return "No flags";
	const highest = highestRisk(risks);
	const suffix = risks.length === 1 ? "flag" : "flags";
	return `${highest?.level.toUpperCase()}: ${risks.length} ${suffix}`;
}

export function subjectLabel(subject: {
	kind: string;
	name: string;
	namespace?: string;
}): string {
	if (subject.namespace) return `${subject.kind}:${subject.namespace}/${subject.name}`;
	return `${subject.kind}:${subject.name}`;
}

export function subjectListLabel(
	subjects: Array<{ kind: string; name: string; namespace?: string }>,
	limit = 4,
): string {
	if (subjects.length === 0) return "-";
	const visible = subjects.slice(0, limit).map(subjectLabel).join(", ");
	if (subjects.length <= limit) return visible;
	return `${visible}, +${subjects.length - limit} more`;
}

export function uniqueRisks(risks: RbacRiskIndicator[]): RbacRiskIndicator[] {
	const seen = new Set<string>();
	return risks.filter((risk) => {
		const key = `${risk.level}:${risk.label}`;
		if (seen.has(key)) return false;
		seen.add(key);
		return true;
	});
}

export function collectInspectionRisks(
	inspection: RbacInspectionSummary,
): RbacRiskIndicator[] {
	return uniqueRisks([
		...inspection.serviceAccounts.flatMap((item) => item.risks),
		...inspection.roles.flatMap((item) => item.risks),
		...inspection.clusterRoles.flatMap((item) => item.risks),
		...inspection.roleBindings.flatMap((item) => item.risks),
		...inspection.clusterRoleBindings.flatMap((item) => item.risks),
	]);
}

export function riskyCount(
	items: Array<ServiceAccountSummary | RbacRoleSummary | RbacBindingSummary>,
): number {
	return items.filter((item) => item.risks.length > 0).length;
}

export function riskBadgeClass(risks: RbacRiskIndicator[]): string {
	if (risks.some((risk) => risk.level === "high")) {
		return "border-destructive/35 bg-destructive/10 text-destructive";
	}
	if (risks.some((risk) => risk.level === "medium")) {
		return "border-amber-500/35 bg-amber-500/10 text-amber-600 dark:text-amber-300";
	}
	if (risks.some((risk) => risk.level === "unknown")) {
		return "border-muted-foreground/35 bg-muted text-muted-foreground";
	}
	return "";
}

export function riskRailClass(risks: RbacRiskIndicator[]): string {
	if (risks.some((risk) => risk.level === "high")) return "border-l-destructive";
	if (risks.some((risk) => risk.level === "medium")) return "border-l-amber-500";
	if (risks.some((risk) => risk.level === "low")) return "border-l-sky-500";
	if (risks.some((risk) => risk.level === "unknown")) return "border-l-muted-foreground";
	return "border-l-emerald-500";
}
