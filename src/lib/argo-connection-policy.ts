export type ArgoConnectionPreference =
	| { kind: "automatic" }
	| { kind: "kubernetes" }
	| { kind: "connected"; profileId: string };

export interface ArgoConnectionProfilePolicyInput {
	id: string;
	url: string;
	clusterContext?: string | null;
	workspaceId?: string | null;
}

interface ArgoConnectionStatusPolicyInput {
	connected: boolean;
}

export interface ArgoConnectionChoice<T extends ArgoConnectionProfilePolicyInput> {
	preference: ArgoConnectionPreference;
	eligibleProfiles: T[];
	transport: "connected" | "kubernetes";
	connectionId: string | null;
	selectedProfile: T | null;
	unavailable: boolean;
}

export const automaticArgoConnection: ArgoConnectionPreference = { kind: "automatic" };
export const kubernetesArgoConnection: ArgoConnectionPreference = { kind: "kubernetes" };

export function normalizeArgoConnectionPreference(value: unknown): ArgoConnectionPreference {
	if (value === "automatic" || (isRecord(value) && value.kind === "automatic")) {
		return automaticArgoConnection;
	}
	if (value === "kubernetes" || (isRecord(value) && value.kind === "kubernetes")) {
		return kubernetesArgoConnection;
	}
	if (typeof value === "string" && value.startsWith("connected:")) {
		const profileId = value.slice("connected:".length).trim();
		if (profileId) return { kind: "connected", profileId };
	}
	if (
		isRecord(value) &&
		value.kind === "connected" &&
		typeof value.profileId === "string" &&
		value.profileId.trim()
	) {
		return { kind: "connected", profileId: value.profileId.trim() };
	}
	return automaticArgoConnection;
}

export function argoConnectionPreferenceValue(
	preference: ArgoConnectionPreference,
): string {
	return preference.kind === "connected"
		? `connected:${preference.profileId}`
		: preference.kind;
}

export function eligibleArgoProfiles<T extends ArgoConnectionProfilePolicyInput>(
	profiles: readonly T[],
	clusterContext: string,
	workspaceId: string,
): T[] {
	return profiles.filter(
		(profile) =>
			profile.clusterContext === clusterContext && profile.workspaceId === workspaceId,
	);
}

export function upsertArgoProfileInSavedOrder<
	T extends ArgoConnectionProfilePolicyInput,
>(profiles: readonly T[], profile: T, previousId = profile.id): T[] {
	const index = profiles.findIndex(
		(candidate) => candidate.id === previousId || candidate.id === profile.id,
	);
	if (index < 0) return [...profiles, profile];
	return profiles.flatMap((candidate, candidateIndex) => {
		if (candidateIndex === index) return [profile];
		return candidate.id === profile.id ? [] : [candidate];
	});
}

export function resolveArgoConnectionPolicy<
	T extends ArgoConnectionProfilePolicyInput,
>({
	profiles,
	statuses,
	clusterContext,
	workspaceId,
	preference,
}: {
	profiles: readonly T[];
	statuses?:
		| ReadonlyMap<string, ArgoConnectionStatusPolicyInput>
		| readonly (readonly [string, ArgoConnectionStatusPolicyInput])[];
	clusterContext: string;
	workspaceId: string;
	preference?: unknown;
}): ArgoConnectionChoice<T> {
	const eligibleProfiles = eligibleArgoProfiles(profiles, clusterContext, workspaceId);
	const normalized = normalizeArgoConnectionPreference(preference);
	const statusMap = statuses instanceof Map ? statuses : new Map(statuses ?? []);
	const healthy = (profile: T) => statusMap.get(profile.id)?.connected === true;

	if (normalized.kind === "kubernetes") {
		return result(normalized, eligibleProfiles, "kubernetes", null, null, false);
	}
	if (normalized.kind === "connected") {
		const profile =
			eligibleProfiles.find((candidate) => candidate.id === normalized.profileId) ?? null;
		return result(
			normalized,
			eligibleProfiles,
			"connected",
			normalized.profileId,
			profile,
			!profile || !healthy(profile),
		);
	}
	const profile = eligibleProfiles.find(healthy) ?? null;
	return result(
		normalized,
		eligibleProfiles,
		profile ? "connected" : "kubernetes",
		profile?.id ?? null,
		profile,
		false,
	);
}

function result<T extends ArgoConnectionProfilePolicyInput>(
	preference: ArgoConnectionPreference,
	eligibleProfiles: T[],
	transport: "connected" | "kubernetes",
	connectionId: string | null,
	selectedProfile: T | null,
	unavailable: boolean,
): ArgoConnectionChoice<T> {
	return {
		preference,
		eligibleProfiles,
		transport,
		connectionId,
		selectedProfile,
		unavailable,
	};
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null;
}
