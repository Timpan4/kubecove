import type { ResourceSummary } from "./types";

export interface HelmReleaseSummary {
	cluster: string;
	name: string;
	namespace: string;
	age: string;
	updatedAt?: string;
	createdAt?: string;
	chart?: string;
	appVersion?: string;
	revision?: number;
	status?: string;
	storageKind: string;
	storageName: string;
}

export interface HelmReleaseDetails {
	summary: HelmReleaseSummary;
	yaml: string;
	metadata: Record<string, unknown>;
	valuesSummary: HelmValuesSummary;
	manifestSummary: HelmManifestSummary;
	release?: Record<string, unknown>;
}

export type HelmReconciliationStatus =
	| "tracked"
	| "unlabeledLive"
	| "missing"
	| "labelOnly"
	| "unavailable";

export interface HelmReleaseReconciliation {
	summary: HelmReleaseSummary;
	totals: HelmReconciliationTotals;
	resources: HelmReconciliationResource[];
	warnings: string[];
}

export interface HelmReconciliationTotals {
	tracked: number;
	unlabeledLive: number;
	missing: number;
	labelOnly: number;
	unavailable: number;
}

export interface HelmReconciliationResource {
	apiVersion?: string;
	kind?: string;
	namespace?: string;
	name?: string;
	status: HelmReconciliationStatus;
	statusMessage: string;
	inManifest: boolean;
	explicitHelmLabel: boolean;
	liveResource?: ResourceSummary;
}

export interface HelmValuesSummary {
	hasValues: boolean;
	topLevelKeys: string[];
	valueCount: number;
}

export interface HelmManifestSummary {
	resourceCount: number;
	resources: HelmManifestResourceSummary[];
	truncated: boolean;
}

export interface HelmManifestResourceSummary {
	apiVersion?: string;
	kind?: string;
	name?: string;
	namespace?: string;
}
