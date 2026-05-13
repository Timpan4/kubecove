import { useQuery } from "@tanstack/react-query";
import { memo, useEffect, useMemo, useRef, useState } from "react";
import {
	getResourceDetails,
	getResourceYaml,
	createTauriClient,
} from "../../lib/tauri";
import type { ResourceSummary } from "../../lib/types";
import { diagnosticLog, diagnosticResultSummary } from "../../lib/diagnostics";

interface ResourceDetailPanelProps {
	resource: ResourceSummary;
	onClose: () => void;
}

type Tab = "details" | "yaml";

interface ConditionRow {
	type: string;
	status: string;
	reason?: string;
	message?: string;
}

export function shouldFetchResourceDetails(
	resource: Pick<ResourceSummary, "cluster" | "kind" | "name">,
): boolean {
	return (
		Boolean(resource.cluster) &&
		Boolean(resource.kind) &&
		Boolean(resource.name)
	);
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function getConditionRows(
	status: Record<string, unknown> | undefined,
): ConditionRow[] {
	if (!status || !Array.isArray(status.conditions)) return [];
	return status.conditions.filter(isRecord).map((condition) => ({
		type: String(condition.type ?? "Condition"),
		status: String(condition.status ?? "Unknown"),
		reason:
			typeof condition.reason === "string" ? condition.reason : undefined,
		message:
			typeof condition.message === "string" ? condition.message : undefined,
	}));
}

function DetailField({
	label,
	value,
}: {
	label: string;
	value: string | undefined | null;
}) {
	if (!value) return null;
	return (
		<div className="detail-row">
			<span className="detail-key">{label}</span>
			<span className="detail-value">{value}</span>
		</div>
	);
}

function StatusChip({
	value,
	label,
}: {
	value: string | undefined;
	label: string;
}) {
	if (!value) return null;
	const variant =
		value === "Running" || value === "Succeeded" || value === "Ready"
			? "success"
			: value === "Pending" || value === "Terminating"
				? "warning"
				: value === "Failed" || value === "Error"
					? "error"
					: "neutral";
	return (
		<div className="detail-row">
			<span className="detail-key">{label}</span>
			<span className="detail-value">
				<span className={`chip chip-${variant}`}>{value}</span>
			</span>
		</div>
	);
}

function ConditionList({ conditions }: { conditions: ConditionRow[] }) {
	if (conditions.length === 0) return null;
	return (
		<div className="detail-section">
			<div className="detail-section-title">Conditions</div>
			<div className="condition-list">
				{conditions.map((condition) => (
					<div className="condition-row" key={`${condition.type}:${condition.status}`}>
						<div className="condition-row-header">
							<span className="condition-type">{condition.type}</span>
							<span
								className={`chip chip-${
									condition.status === "True"
										? "success"
										: condition.status === "False"
											? "error"
											: "warning"
								}`}
							>
								{condition.status}
							</span>
						</div>
						{condition.reason && (
							<div className="condition-reason">{condition.reason}</div>
						)}
						{condition.message && (
							<div className="condition-message">{condition.message}</div>
						)}
					</div>
				))}
			</div>
		</div>
	);
}

function BadgeRow({
	argoApp,
	helmRelease,
}: {
	argoApp?: string;
	helmRelease?: string;
}) {
	if (!argoApp && !helmRelease) return null;
	return (
		<div className="detail-row">
			<span className="detail-key">App</span>
			<span className="detail-value">
				<div className="detail-badges">
					{argoApp && <span className="badge badge-argo">Argo: {argoApp}</span>}
					{helmRelease && (
						<span className="badge badge-helm">Helm: {helmRelease}</span>
					)}
				</div>
			</span>
		</div>
	);
}

export const ResourceDetailPanel = memo(function ResourceDetailPanel({
	resource,
	onClose,
}: ResourceDetailPanelProps) {
	const [activeTab, setActiveTab] = useState<Tab>("details");
	const client = useMemo(() => createTauriClient(), []);
	const resourceKey = `${resource.cluster}:${resource.kind}:${resource.namespace ?? ""}:${resource.name}`;
	const renderCountRef = useRef(0);
	renderCountRef.current += 1;

	// Reset state when viewing a different resource
	useEffect(() => {
		diagnosticLog("detail.resource.changed", {
			key: resourceKey,
			render: renderCountRef.current,
		});
		setActiveTab("details");
	}, [resourceKey]);

	useEffect(() => {
		diagnosticLog("detail.mount", { key: resourceKey });
		return () => {
			diagnosticLog("detail.unmount", { key: resourceKey });
		};
	}, [resourceKey]);

	const detailsEnabled = shouldFetchResourceDetails(resource);
	const yamlEnabled =
		activeTab === "yaml" &&
		!!resource.cluster &&
		!!resource.kind &&
		!!resource.name;

	const {
		data: details,
		isLoading: detailsLoading,
		isError: detailsError,
		error: detailsErr,
	} = useQuery({
		queryKey: [
			"resource-details",
			resource.cluster,
			resource.kind,
			resource.name,
			resource.namespace,
		],
		queryFn: async () => {
			const started = performance.now();
			diagnosticLog("detail.details.fetch.start", { key: resourceKey });
			const result = await getResourceDetails(
				client,
				resource.cluster,
				resource.kind,
				resource.name,
				resource.namespace ?? undefined,
			);
			diagnosticLog("detail.details.fetch.done", {
				key: resourceKey,
				ms: Math.round(performance.now() - started),
				result: diagnosticResultSummary(result),
			});
			return result;
		},
		enabled: detailsEnabled,
		retry: false,
	});

	const {
		data: yaml,
		isLoading: yamlLoading,
		isError: yamlError,
		error: yamlErr,
	} = useQuery({
		queryKey: [
			"resource-yaml",
			resource.cluster,
			resource.kind,
			resource.name,
			resource.namespace,
		],
		queryFn: async () => {
			const started = performance.now();
			diagnosticLog("detail.yaml.fetch.start", { key: resourceKey });
			const result = await getResourceYaml(
				client,
				resource.cluster,
				resource.kind,
				resource.name,
				resource.namespace ?? undefined,
			);
			diagnosticLog("detail.yaml.fetch.done", {
				key: resourceKey,
				ms: Math.round(performance.now() - started),
				result: diagnosticResultSummary(result),
			});
			return result;
		},
		enabled: yamlEnabled,
		retry: false,
	});

	useEffect(() => {
		diagnosticLog("detail.render", {
			key: resourceKey,
			render: renderCountRef.current,
			tab: activeTab,
			detailsEnabled,
			detailsLoading,
			yamlEnabled,
			yamlLoading,
			hasDetails: Boolean(details),
			hasYaml: Boolean(yaml),
		});
	});

	const formatMetadata = (
		metadata: Record<string, unknown>,
	): Array<{ key: string; value: unknown }> => {
		const entries: Array<{ key: string; value: unknown }> = [];
		if (metadata.name) entries.push({ key: "Name", value: metadata.name });
		if (metadata.namespace)
			entries.push({ key: "Namespace", value: metadata.namespace });
		if (metadata.uid) entries.push({ key: "UID", value: metadata.uid });
		if (metadata.resourceVersion)
			entries.push({
				key: "Resource Version",
				value: metadata.resourceVersion,
			});
		if (metadata.creationTimestamp)
			entries.push({ key: "Created", value: metadata.creationTimestamp });
		if (metadata.labels)
			entries.push({
				key: "Labels",
				value: JSON.stringify(metadata.labels, null, 2),
			});
		if (metadata.annotations)
			entries.push({
				key: "Annotations",
				value: JSON.stringify(metadata.annotations, null, 2),
			});
		return entries;
	};

	const getErrorMessage = (err: unknown): string => {
		if (err instanceof Error) return err.message;
		if (typeof err === "string") return err;
		return "Unknown error";
	};
	const conditionRows = useMemo(
		() => getConditionRows(details?.status),
		[details?.status],
	);

	return (
		<div className="right-panel">
			<div className="panel-header">
				<span className="panel-header-title">{resource.name}</span>
				<button
					onClick={onClose}
					style={{
						background: "none",
						border: "none",
						color: "#888",
						cursor: "pointer",
						fontSize: "18px",
						padding: "0 4px",
					}}
					aria-label="Close panel"
				>
					×
				</button>
			</div>
			<div className="panel-tabs">
				<button
					className={`panel-tab ${activeTab === "details" ? "active" : ""}`}
					onClick={() => {
						diagnosticLog("detail.tab.click", {
							key: resourceKey,
							tab: "details",
						});
						setActiveTab("details");
					}}
				>
					Details
				</button>
				<button
					className={`panel-tab ${activeTab === "yaml" ? "active" : ""}`}
					onClick={() => {
						diagnosticLog("detail.tab.click", {
							key: resourceKey,
							tab: "yaml",
						});
						setActiveTab("yaml");
					}}
				>
					YAML
				</button>
			</div>
			<div className="panel-body">
				{activeTab === "details" && (
					<>
						<div className="resource-detail-summary">
							<div>
								<span>Kind</span>
								<strong>{resource.kind}</strong>
							</div>
							<div>
								<span>Namespace</span>
								<strong>{resource.namespace ?? "cluster-scoped"}</strong>
							</div>
							{resource.age && (
								<div>
									<span>Age</span>
									<strong>{resource.age}</strong>
								</div>
							)}
						</div>

						<div className="detail-section">
							<div className="detail-section-title">Status</div>
							<StatusChip value={resource.status} label="Phase" />
							<StatusChip value={resource.ready} label="Ready" />
							{resource.restarts !== undefined && resource.restarts > 0 && (
								<div className="detail-row">
									<span className="detail-key">Restarts</span>
									<span className="detail-value">
										<span
											className={`chip chip-${resource.restarts > 5 ? "error" : "warning"}`}
										>
											{resource.restarts}
										</span>
									</span>
								</div>
							)}
						</div>

						<div className="detail-section">
							<div className="detail-section-title">Ownership</div>
							<DetailField label="Owner" value={resource.ownerRef} />
							<BadgeRow
								argoApp={resource.argoApp}
								helmRelease={resource.helmRelease}
							/>
						</div>

						{detailsLoading && (
							<div className="loading-state">
								<div
									className="loading-spinner"
									style={{ width: "16px", height: "16px", marginBottom: "8px" }}
								></div>
								<span style={{ fontSize: "12px" }}>Loading details...</span>
							</div>
						)}
						{detailsError && (
							<div className="error-state">
								<p>Error loading details: {getErrorMessage(detailsErr)}</p>
							</div>
						)}
						{!detailsLoading && !detailsError && details && (
							<>
								<ConditionList conditions={conditionRows} />
								{details.status && (
									<div className="detail-section">
										<div className="detail-section-title">Status Details</div>
										<pre className="status-json-block">
											{JSON.stringify(details.status, null, 2)}
										</pre>
									</div>
								)}

								<div className="detail-section">
									<div className="detail-section-title">Metadata</div>
									{formatMetadata(
										details.metadata as Record<string, unknown>,
									).map(({ key, value }) => (
										<div key={key} className="detail-row">
											<span className="detail-key">{key}</span>
											<span className="detail-value">
												{typeof value === "string"
													? value
													: JSON.stringify(value)}
											</span>
										</div>
									))}
								</div>
							</>
						)}
					</>
				)}
				{activeTab === "yaml" && (
					<>
						{yamlLoading && (
							<div className="loading-state">
								<div
									className="loading-spinner"
									style={{ width: "16px", height: "16px", marginBottom: "8px" }}
								></div>
								<span style={{ fontSize: "12px" }}>Loading YAML...</span>
							</div>
						)}
						{yamlError && (
							<div className="error-state">
								<p>Error loading YAML: {getErrorMessage(yamlErr)}</p>
							</div>
						)}
						{!yamlLoading && !yamlError && yaml && (
							<pre className="yaml-block">{yaml}</pre>
						)}
					</>
				)}
			</div>
		</div>
	);
});
