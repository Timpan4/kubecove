import type {
	RbacBindingSummary,
	RbacInspectionSummary,
	RbacRiskIndicator,
	RbacSubjectSummary,
} from "@/lib/types";
import type { TreeNodeId } from "@/lib/tree-nav";
import { collectInspectionRisks, riskyCount } from "@/features/rbac/risk";

export type RbacView =
	| "Namespace Access"
	| "Roles"
	| "Cluster Roles"
	| "Bindings"
	| "Service Accounts";

export interface RbacTable {
	headers: string[];
	rows: string[][];
	empty: string;
}

export type RbacStat = [label: string, value: number];

const RBAC_VIEWS = new Set<RbacView>([
	"Namespace Access",
	"Roles",
	"Cluster Roles",
	"Bindings",
	"Service Accounts",
]);

export function selectedRbacView(
	selectedNode: TreeNodeId | null,
): RbacView {
	if (
		selectedNode?.type === "kind" &&
		selectedNode.section === "rbac" &&
		RBAC_VIEWS.has(selectedNode.kind as RbacView)
	) {
		return selectedNode.kind as RbacView;
	}
	return "Namespace Access";
}

export function buildRbacTable(
	data: RbacInspectionSummary,
	view: RbacView,
): RbacTable {
	if (view === "Roles") {
		return {
			headers: ["Kind", "Name", "Namespace", "Rules", "Risks"],
			rows: data.roles.map((item) => [
				item.kind,
				item.name,
				item.namespace ?? "-",
				String(item.rulesCount),
				riskLabels(item.risks),
			]),
			empty: "No Roles found",
		};
	}
	if (view === "Cluster Roles") {
		return {
			headers: ["Kind", "Name", "Rules", "Risks"],
			rows: data.clusterRoles.map((item) => [
				item.kind,
				item.name,
				String(item.rulesCount),
				riskLabels(item.risks),
			]),
			empty: "No ClusterRoles found",
		};
	}
	if (view === "Bindings") {
		return {
			headers: ["Kind", "Name", "Namespace", "Role", "Subjects", "Risks"],
			rows: [...data.roleBindings, ...data.clusterRoleBindings].map(bindingRow),
			empty: "No RBAC bindings found",
		};
	}
	if (view === "Service Accounts") {
		return {
			headers: ["Name", "Namespace", "Secrets", "Image pulls", "Automount", "Risks"],
			rows: data.serviceAccounts.map((item) => [
				item.name,
				item.namespace,
				String(item.secretsCount),
				String(item.imagePullSecretsCount),
				item.automountToken === undefined ? "-" : String(item.automountToken),
				riskLabels(item.risks),
			]),
			empty: "No ServiceAccounts found",
		};
	}
	return {
		headers: ["Namespace", "Service accounts", "Roles", "Bindings", "Subjects", "Risks"],
		rows: data.namespaceAccess.map((item) => [
			item.namespace,
			String(item.serviceAccounts),
			String(item.roles),
			String(item.roleBindings),
			String(item.boundSubjects.length),
			riskLabels(item.risks),
		]),
		empty: "No RBAC namespace access rows found",
	};
}

export function buildRbacStats(data: RbacInspectionSummary): RbacStat[] {
	return [
		["Service accounts", data.serviceAccounts.length],
		["Roles", data.roles.length],
		["Cluster roles", data.clusterRoles.length],
		["Bindings", data.roleBindings.length + data.clusterRoleBindings.length],
		["Risk flags", collectInspectionRisks(data).length],
		[
			"Flagged objects",
			riskyCount(data.serviceAccounts) +
				riskyCount(data.roles) +
				riskyCount(data.clusterRoles) +
				riskyCount(data.roleBindings) +
				riskyCount(data.clusterRoleBindings),
		],
	];
}

export function rbacWarningSummary(warnings: string[]): string {
	return `${warnings.slice(0, 3).join(" ")}${warnings.length > 3 ? ` ${warnings.length - 3} more warnings.` : ""}`;
}

function bindingRow(item: RbacBindingSummary): string[] {
	return [
		item.kind,
		item.name,
		item.namespace ?? "-",
		`${item.roleRefKind}/${item.roleRefName}`,
		subjectLabels(item.subjects),
		riskLabels(item.risks),
	];
}

function subjectLabels(subjects: RbacSubjectSummary[]): string {
	if (subjects.length === 0) return "-";
	return subjects
		.slice(0, 3)
		.map((subject) =>
			subject.namespace
				? `${subject.kind}/${subject.namespace}/${subject.name}`
				: `${subject.kind}/${subject.name}`,
		)
		.join(", ");
}

function riskLabels(risks: RbacRiskIndicator[]): string {
	if (risks.length === 0) return "-";
	return risks.map((risk) => `${risk.level}: ${risk.label}`).join(", ");
}
