import { StatusBadge } from "@/components/StatusBadge";
import { TimestampText } from "@/components/TimestampText";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import type {
	RbacBindingSummary,
	RbacNamespaceAccessSummary,
	RbacRiskIndicator,
	RbacRoleSummary,
	ServiceAccountSummary,
} from "@/lib/types";
import { riskSummaryLabel, riskTone, subjectLabel } from "./risk";

const TABLE_CLASS =
	"w-full table-fixed border-collapse text-sm [&_th]:border-b-2 [&_th]:px-3 [&_th]:py-3 [&_th]:text-left [&_th]:text-xs [&_th]:font-semibold [&_th]:uppercase [&_th]:text-muted-foreground [&_td]:border-b [&_td]:px-3 [&_td]:py-3";
const MUTED_CELL = "min-w-0 truncate text-muted-foreground";

function RiskBadges({ risks }: { risks: RbacRiskIndicator[] }) {
	if (risks.length === 0) {
		return <StatusBadge tone="success">No flags</StatusBadge>;
	}
	return (
		<div className="flex min-w-0 flex-wrap gap-1">
			{risks.slice(0, 3).map((risk) => (
				<StatusBadge
					key={`${risk.level}:${risk.label}`}
					tone={riskTone(risk.level)}
					className="max-w-full truncate"
				>
					{risk.label}
				</StatusBadge>
			))}
			{risks.length > 3 && (
				<StatusBadge tone="neutral">+{risks.length - 3}</StatusBadge>
			)}
		</div>
	);
}

export function NamespaceAccessTable({
	rows,
}: {
	rows: RbacNamespaceAccessSummary[];
}) {
	return (
		<Table className={TABLE_CLASS}>
			<TableHeader>
				<TableRow>
					<TableHead>Namespace</TableHead>
					<TableHead>Service Accounts</TableHead>
					<TableHead>Roles</TableHead>
					<TableHead>Bindings</TableHead>
					<TableHead>Bound Subjects</TableHead>
					<TableHead>Risk</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				{rows.map((row) => (
					<TableRow key={row.namespace}>
						<TableCell className="font-medium">{row.namespace}</TableCell>
						<TableCell>{row.serviceAccounts}</TableCell>
						<TableCell>{row.roles}</TableCell>
						<TableCell>{row.roleBindings}</TableCell>
						<TableCell className={MUTED_CELL}>
							{row.boundSubjects.slice(0, 4).map(subjectLabel).join(", ") || "-"}
						</TableCell>
						<TableCell>
							<RiskBadges risks={row.risks} />
						</TableCell>
					</TableRow>
				))}
			</TableBody>
		</Table>
	);
}

export function RolesTable({ rows }: { rows: RbacRoleSummary[] }) {
	return (
		<Table className={TABLE_CLASS}>
			<TableHeader>
				<TableRow>
					<TableHead>Name</TableHead>
					<TableHead>Kind</TableHead>
					<TableHead>Namespace</TableHead>
					<TableHead>Rules</TableHead>
					<TableHead>Risk</TableHead>
					<TableHead>Age</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				{rows.map((row) => (
					<TableRow key={`${row.kind}:${row.namespace ?? ""}:${row.name}`}>
						<TableCell className="font-medium">{row.name}</TableCell>
						<TableCell>{row.kind}</TableCell>
						<TableCell className={MUTED_CELL}>{row.namespace ?? "-"}</TableCell>
						<TableCell>{row.rulesCount}</TableCell>
						<TableCell title={riskSummaryLabel(row.risks)}>
							<RiskBadges risks={row.risks} />
						</TableCell>
						<TableCell>
							<TimestampText relative={row.age} exact={row.createdAt} />
						</TableCell>
					</TableRow>
				))}
			</TableBody>
		</Table>
	);
}

export function BindingsTable({ rows }: { rows: RbacBindingSummary[] }) {
	return (
		<Table className={TABLE_CLASS}>
			<TableHeader>
				<TableRow>
					<TableHead>Name</TableHead>
					<TableHead>Kind</TableHead>
					<TableHead>Namespace</TableHead>
					<TableHead>Role Ref</TableHead>
					<TableHead>Subjects</TableHead>
					<TableHead>Risk</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				{rows.map((row) => (
					<TableRow key={`${row.kind}:${row.namespace ?? ""}:${row.name}`}>
						<TableCell className="font-medium">{row.name}</TableCell>
						<TableCell>{row.kind}</TableCell>
						<TableCell className={MUTED_CELL}>{row.namespace ?? "-"}</TableCell>
						<TableCell className={MUTED_CELL}>
							{row.roleRefKind}/{row.roleRefName}
						</TableCell>
						<TableCell className={MUTED_CELL}>
							{row.subjects.slice(0, 4).map(subjectLabel).join(", ") || "-"}
						</TableCell>
						<TableCell>
							<RiskBadges risks={row.risks} />
						</TableCell>
					</TableRow>
				))}
			</TableBody>
		</Table>
	);
}

export function ServiceAccountsTable({
	rows,
}: {
	rows: ServiceAccountSummary[];
}) {
	return (
		<Table className={TABLE_CLASS}>
			<TableHeader>
				<TableRow>
					<TableHead>Name</TableHead>
					<TableHead>Namespace</TableHead>
					<TableHead>Automount</TableHead>
					<TableHead>Secrets</TableHead>
					<TableHead>Image Pull Secrets</TableHead>
					<TableHead>Risk</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				{rows.map((row) => (
					<TableRow key={`${row.namespace}:${row.name}`}>
						<TableCell className="font-medium">{row.name}</TableCell>
						<TableCell>{row.namespace}</TableCell>
						<TableCell>{row.automountToken === undefined ? "-" : String(row.automountToken)}</TableCell>
						<TableCell>{row.secretsCount}</TableCell>
						<TableCell>{row.imagePullSecretsCount}</TableCell>
						<TableCell>
							<RiskBadges risks={row.risks} />
						</TableCell>
					</TableRow>
				))}
			</TableBody>
		</Table>
	);
}
