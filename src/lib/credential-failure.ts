import type { CredentialFailure } from "./types";

// ADR 0037: Rust sends this only for the desktop user's own kubeconfig, with
// the plugin line already redacted. Any other shape is ignored.

function isRecord(value: unknown): value is Record<string, unknown> {
	return value !== null && Object(value) === value;
}

function text(value: unknown): value is string {
	// oxlint-disable-next-line anti-slop/no-runtime-typeof -- Error payloads are untrusted JSON.
	return typeof value === "string";
}

function integer(value: unknown): value is number {
	return Number.isInteger(value);
}

export function credentialFailureFrom(cause: unknown): CredentialFailure | undefined {
	if (!isRecord(cause) || cause.kind !== "credentialUnavailable") return undefined;
	const value = cause.credential;
	if (!isRecord(value)) return undefined;
	const { context, command, reproduce, kubeconfig, user, exitStatus, pluginMessage } = value;
	if (!text(context) || !text(command) || !text(reproduce)) return undefined;
	if (!Array.isArray(kubeconfig) || !kubeconfig.every(text)) return undefined;
	return {
		context,
		command,
		reproduce,
		kubeconfig,
		user: text(user) ? user : undefined,
		exitStatus: integer(exitStatus) ? exitStatus : undefined,
		pluginMessage: text(pluginMessage) ? pluginMessage : undefined,
	};
}

export const CREDENTIAL_NEXT_STEP =
	"Run the sign-in command in a terminal to see its full output, fix the sign-in, then retry.";

export function credentialTitle(failure: CredentialFailure): string {
	return `Could not sign in to ${failure.context}`;
}

export function credentialSummary(failure: CredentialFailure): string {
	return failure.exitStatus === undefined
		? `The sign-in command ${failure.command} failed.`
		: `The sign-in command ${failure.command} exited with status ${failure.exitStatus}.`;
}

/** The label and value rows the panel lists, in display order. */
export function credentialDetails(failure: CredentialFailure): Array<[string, string]> {
	const rows: Array<[string, string]> = [["Context", failure.context]];
	if (failure.user !== undefined) rows.push(["User", failure.user]);
	rows.push(["Command", failure.command]);
	for (const path of failure.kubeconfig) rows.push(["Kubeconfig", path]);
	return rows;
}
