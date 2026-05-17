import { MetadataBadges } from "@/components/MetadataBadges";
import { StatusBadge, type StatusTone } from "@/components/StatusBadge";
import { ExactTimestampText } from "@/components/TimestampText";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Spinner } from "@/components/ui/spinner";

export const DETAIL_SECTION_CLASS = "mb-4";
export const DETAIL_SECTION_TITLE_CLASS =
	"mb-2 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground";
export const DETAIL_ROW_CLASS = "flex gap-3 border-b py-1.5";
export const DETAIL_KEY_CLASS = "min-w-[120px] text-xs font-medium text-muted-foreground";
export const DETAIL_VALUE_CLASS = "min-w-0 flex-1 wrap-anywhere text-xs text-foreground";
export const YAML_BLOCK_CLASS =
	"whitespace-pre-wrap break-normal font-mono text-xs leading-relaxed text-foreground [overflow-wrap:anywhere]";
export const JSON_BLOCK_CLASS =
	"whitespace-pre-wrap font-mono text-xs text-foreground [overflow-wrap:anywhere]";
export const DETAIL_HINT_CLASS = "mb-4 text-xs text-muted-foreground";

export function DetailField({
	label,
	value,
}: {
	label: string;
	value: string | undefined | null;
}) {
	if (!value) return null;
	return (
		<div className={DETAIL_ROW_CLASS}>
			<span className={DETAIL_KEY_CLASS}>{label}</span>
			<span className={DETAIL_VALUE_CLASS}>{value}</span>
		</div>
	);
}

export function DetailStatusField({
	value,
	label,
	tone,
}: {
	value: string | null | undefined;
	label: string;
	tone: StatusTone;
}) {
	if (!value) return null;
	return (
		<div className={DETAIL_ROW_CLASS}>
			<span className={DETAIL_KEY_CLASS}>{label}</span>
			<span className={DETAIL_VALUE_CLASS}>
				<StatusBadge tone={tone}>{value}</StatusBadge>
			</span>
		</div>
	);
}

export function DetailLoadingState({ label }: { label: string }) {
	return (
		<div className="p-6 text-center text-xs text-muted-foreground">
			<Spinner className="mx-auto mb-2 size-4" />
			<span>{label}</span>
		</div>
	);
}

export function DetailErrorState({
	title,
	error,
}: {
	title: string;
	error: unknown;
}) {
	return (
		<Alert variant="destructive">
			<AlertTitle>{title}</AlertTitle>
			<AlertDescription>{getErrorMessage(error)}</AlertDescription>
		</Alert>
	);
}

export function DetailMetadata({
	metadata,
}: {
	metadata: Record<string, unknown>;
}) {
	return (
		<div className={DETAIL_SECTION_CLASS}>
			<div className={DETAIL_SECTION_TITLE_CLASS}>Metadata</div>
			{formatMetadata(metadata).map(({ key, value }) => (
				<div key={key} className={DETAIL_ROW_CLASS}>
					<span className={DETAIL_KEY_CLASS}>{key}</span>
					<span className={DETAIL_VALUE_CLASS}>
						{key === "Labels" || key === "Annotations" ? (
							<MetadataBadges value={value} />
						) : key === "Created" && typeof value === "string" ? (
							<ExactTimestampText value={value} />
						) : typeof value === "string" ? (
							value
						) : (
							JSON.stringify(value)
						)}
					</span>
				</div>
			))}
		</div>
	);
}

export function getErrorMessage(err: unknown): string {
	if (err instanceof Error) return err.message;
	if (typeof err === "string") return err;
	return "Unknown error";
}

export function syncStatusTone(status: string | null | undefined): StatusTone {
	if (status === "Synced") return "success";
	if (status === "OutOfSync") return "warning";
	return "neutral";
}

export function healthStatusTone(status: string | null | undefined): StatusTone {
	if (status === "Healthy") return "success";
	if (status === "Degraded" || status === "Missing") return "error";
	return "warning";
}

function formatMetadata(
	metadata: Record<string, unknown>,
): Array<{ key: string; value: unknown }> {
	const entries: Array<{ key: string; value: unknown }> = [];
	if (metadata.name) entries.push({ key: "Name", value: metadata.name });
	if (metadata.namespace)
		entries.push({ key: "Namespace", value: metadata.namespace });
	if (metadata.uid) entries.push({ key: "UID", value: metadata.uid });
	if (metadata.resourceVersion)
		entries.push({ key: "Resource Version", value: metadata.resourceVersion });
	if (metadata.creationTimestamp)
		entries.push({ key: "Created", value: metadata.creationTimestamp });
	if (metadata.labels) entries.push({ key: "Labels", value: metadata.labels });
	if (metadata.annotations)
		entries.push({ key: "Annotations", value: metadata.annotations });
	return entries;
}
