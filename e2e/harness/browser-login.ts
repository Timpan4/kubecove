import { BROWSER_SESSION_PATH, BROWSER_SESSION_STORAGE_KEY } from "../../src/lib/browser-session-contract";

export const browserLoginConfig = (token: string) => ({
	token,
	sessionPath: BROWSER_SESSION_PATH,
	storageKey: BROWSER_SESSION_STORAGE_KEY,
});

export async function storeBrowserSession({ token, sessionPath, storageKey }: ReturnType<typeof browserLoginConfig>) {
	const response = await fetch(sessionPath, {
		method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ token }),
	});
	if (!response.ok) throw new Error(`Browser session login returned HTTP ${response.status}`);
	const { resumeToken } = await response.json();
	// oxlint-disable-next-line anti-slop/no-runtime-typeof -- Validate the session reply before storing its token.
	if (typeof resumeToken !== "string" || !resumeToken) throw new Error("Browser session login did not return a resume token");
	sessionStorage.setItem(storageKey, resumeToken);
}
