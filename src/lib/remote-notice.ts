//! The notice for a selected remote that is not connected (ADR 0031, "Session notices in the
//! native UI"). It mirrors `kubecove_host::remote::RemoteConnectionSummary::notice`, and the
//! shared parity scenarios hold both clients to the same text.

import type { RemoteConnectionSummary } from "./remote-runtime";

export type RemoteNoticeAction = "signIn" | "copySignInLink" | "cancelSignIn" | "startNewSession" | "cancelConnecting" | "disconnect";

export interface RemoteNotice {
	title: string | null;
	message: string;
	actions: readonly RemoteNoticeAction[];
}

export const REMOTE_NOTICE_LABELS: Record<RemoteNoticeAction, string> = {
	signIn: "Sign in",
	copySignInLink: "Copy sign-in link",
	cancelSignIn: "Cancel",
	startNewSession: "Start a new session",
	cancelConnecting: "Cancel",
	disconnect: "Disconnect",
};

export const SIGN_IN_MEMORY_ONLY = "This computer's credential store is unavailable, so the sign-in lasts only until KubeCove closes.";

export const SIGN_IN_NOT_DELETED =
	'This computer\'s credential store could not delete the sign-in, so it may still be stored. KubeCove will not use it while it runs. Remove any "KubeCove remote sign-in" entry for this server from the credential store.';

/** Notes on how this computer keeps the sign-in, shown under a remote's status or notice. */
export function signInNotes(summary: RemoteConnectionSummary): string[] {
	const notes: string[] = [];
	if (summary.signInMemoryOnly) notes.push(SIGN_IN_MEMORY_ONLY);
	if (summary.signInNotDeleted) notes.push(SIGN_IN_NOT_DELETED);
	return notes;
}

const SIGN_IN_AGAIN = "Your sign-in has expired. Sign in again.";

/** The notice for the summary's state, or null while the session is connected. */
export function remoteNotice(summary: RemoteConnectionSummary): RemoteNotice | null {
	switch (summary.sessionState) {
		case "connected":
			return null;
		case "signInRequired":
			return { title: "Sign in to KubeCove", message: "Sign in through your browser to use this server.", actions: ["signIn", "disconnect"] };
		case "connecting":
			return { title: "Connecting to KubeCove", message: "Opening an authenticated session…", actions: ["cancelConnecting"] };
		case "waitingForBrowser":
			return { title: null, message: "Finish signing in in your browser.", actions: ["copySignInLink", "cancelSignIn", "disconnect"] };
		case "reconnecting":
			return { title: null, message: "Connection interrupted. Reconnecting to the current session.", actions: ["disconnect"] };
		case "sessionEnded":
			return {
				title: "Session ended",
				message: "This app's server session closed or expired. Start a new session to continue. Other sessions on the server are not affected.",
				actions: ["startNewSession", "disconnect"],
			};
		// A token server that refuses its token, or whose grace window elapsed, takes the token again in Settings.
		case "signInExpired":
		case "sessionExpired":
			return summary.authMode === "token"
				? { title: null, message: "The server session expired. Reconnect with the server token.", actions: ["disconnect"] }
				: { title: null, message: SIGN_IN_AGAIN, actions: ["signIn", "disconnect"] };
	}
}

/** The session is open and needs no notice. */
export function isRemoteLive(summary: RemoteConnectionSummary): boolean {
	return summary.connected && remoteNotice(summary) === null;
}
