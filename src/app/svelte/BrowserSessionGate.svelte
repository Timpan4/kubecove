<script lang="ts">
	import { Alert, AlertDescription, AlertTitle, Button, Card, CardContent, CardDescription, CardHeader, CardTitle, Input } from "@/components/ui/svelte";
	import {
		loginToBrowserServer,
		startBrowserOidcSignIn,
		takeOverBrowserSession,
		type BrowserSessionState,
	} from "@/lib/browser-runtime";
	import { LockKeyhole } from "lucide-svelte";

	let { session }: { session: BrowserSessionState } = $props();
	let token = $state("");
	let busy = $state(false);
	let message = $state<string | null>(null);

	async function connect(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		message = null;
		try {
			await loginToBrowserServer(token);
			token = "";
		} catch (error) {
			message = error instanceof Error ? error.message : String(error);
		} finally {
			busy = false;
		}
	}

	async function takeOver() {
		busy = true;
		message = null;
		try {
			await takeOverBrowserSession();
		} catch (error) {
			message = error instanceof Error ? error.message : String(error);
		} finally {
			busy = false;
		}
	}

	async function signIn(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		message = null;
		try {
			await startBrowserOidcSignIn();
		} catch (error) {
			message = error instanceof Error ? error.message : String(error);
			busy = false;
		}
	}
</script>

<main class="flex min-h-screen items-center justify-center bg-background p-4">
	<Card class="w-full max-w-md">
		<CardHeader>
			<div class="mb-2 flex size-10 items-center justify-center rounded-lg border bg-muted text-muted-foreground">
				<LockKeyhole class="size-5" />
			</div>
			<CardTitle>
				{session.status === "checking" || session.status === "connecting"
					? "Connecting to KubeCove"
					: session.status === "moved"
						? "Session ended"
						: session.authenticationMode === "oidc"
							? "Sign in to KubeCove"
							: "Connect to KubeCove"}
			</CardTitle>
			<CardDescription>
				Connect to the KubeCove server on {window.location.host}. Cluster access uses the server host's kubeconfig.
			</CardDescription>
		</CardHeader>
		<CardContent class="flex flex-col gap-4">
			{#if session.status === "checking" || session.status === "connecting"}
				<p class="text-sm text-muted-foreground">Opening an authenticated session…</p>
			{:else}
				{#if session.status === "moved"}
					<Alert>
						<AlertTitle>This browser's server session closed or expired.</AlertTitle>
						<AlertDescription>
							Start a new session to continue. Other sessions on the server are not affected.
						</AlertDescription>
					</Alert>
					<Button type="button" disabled={busy} onclick={takeOver}>
						Start a new session
					</Button>
				{/if}

				{#if session.error || message}
					<div class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
						{message ?? session.error}
					</div>
				{/if}

				{#if session.authenticationMode === "oidc" && session.status !== "moved"}
					<form class="flex flex-col gap-3" onsubmit={signIn}>
						<Button type="submit" disabled={busy}>{busy ? "Opening sign-in…" : "Sign in"}</Button>
					</form>
				{:else if session.authenticationMode === "token"}
					<form class="flex flex-col gap-3" onsubmit={connect}>
						<label class="flex flex-col gap-1.5 text-sm font-medium">
							Server token
							<Input
								bind:value={token}
								type="password"
								autocomplete="off"
								spellcheck={false}
								placeholder="Paste the token from the server"
								disabled={busy}
							/>
						</label>
						<Button type="submit" disabled={busy || !token.trim()}>
							{session.status === "moved" ? "Connect with server token" : "Connect"}
						</Button>
					</form>
				{:else if session.status !== "moved"}
					<Button type="button" onclick={() => window.location.reload()}>Reload to reconnect</Button>
				{/if}
			{/if}
		</CardContent>
	</Card>
</main>
