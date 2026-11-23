import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";
import { text } from "node:stream/consumers";
import test from "node:test";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { readinessTimeoutMs } from "../e2e/harness/timeouts.ts";

const runner = fileURLToPath(new URL("./headless-desktop.sh", import.meta.url));

test("headless desktop changes only its own Secret Service default", { skip: process.platform !== "linux", timeout: readinessTimeoutMs }, async (t) => {
	const caller = mkdtempSync(join(tmpdir(), "kubecove-keyring-caller-"));
	const data = join(caller, "data");
	mkdirSync(data, { mode: 0o700 });
	const runtime = join(caller, "runtime");
	mkdirSync(runtime, { mode: 0o700 });
	const defaultAlias = join(data, "keyrings", "default");

	const child = spawn("dbus-run-session", ["--", "bash", "-euo", "pipefail", "-c", `
		unset GNOME_KEYRING_CONTROL
		printf '%s' fixture | gnome-keyring-daemon --unlock --components=secrets \\
			--control-directory="$XDG_RUNTIME_DIR/keyring" >/dev/null
		login=/org/freedesktop/secrets/collection/login
		dbus-send --session --print-reply --dest=org.freedesktop.secrets \\
			/org/freedesktop/secrets org.freedesktop.Secret.Service.SetAlias \\
			string:default "objpath:$login" >/dev/null
		read_default() {
			dbus-send --session --print-reply --dest=org.freedesktop.secrets \\
				/org/freedesktop/secrets org.freedesktop.Secret.Service.ReadAlias string:default | tail -n 1
		}
		before=$(read_default)
		[[ "$before" == *"object path \\\"$login\\\"" ]]
		status=0
		CALLER_BUS=$DBUS_SESSION_BUS_ADDRESS bash "$1" bash -euc "$2" || status=$?
		after=$(read_default)
		[[ "$after" == "$before" ]]
		exit "$status"
	`, "caller", runner, `
		test "$HOME" != "$CALLER_HOME"
		for directory in "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_STATE_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR" "$TMPDIR"; do
			[[ "$directory" == "$HOME/"* && -d "$directory" ]]
		done
		test -n "$DISPLAY"
		test -z "\${WAYLAND_DISPLAY-}\${SSH_AUTH_SOCK-}"
		test "$DBUS_SESSION_BUS_ADDRESS" != "$CALLER_BUS"
		reply=$(dbus-send --session --print-reply --dest=org.freedesktop.secrets \\
			/org/freedesktop/secrets org.freedesktop.Secret.Service.ReadAlias string:default)
		[[ "$reply" == *'object path "/org/freedesktop/secrets/collection/session"'* ]]
		test "$(cat "$XDG_DATA_HOME/keyrings/default")" = session
		printf '%s\\n' "$HOME"
		exit 23
	`], {
		env: {
			...process.env,
			HOME: caller,
			CALLER_HOME: caller,
			XDG_CONFIG_HOME: join(caller, "config"),
			XDG_DATA_HOME: data,
			XDG_CACHE_HOME: join(caller, "cache"),
			XDG_STATE_HOME: join(caller, "state"),
			XDG_RUNTIME_DIR: runtime,
			TMPDIR: caller,
			WAYLAND_DISPLAY: "caller-wayland",
			SSH_AUTH_SOCK: join(caller, "caller-ssh"),
		},
		detached: true,
	});
	const closed = once(child, "close");
	t.after(async () => {
		if (child.exitCode === null && child.signalCode === null && child.pid !== undefined) {
			try { process.kill(-child.pid, "SIGTERM"); } catch (error) {
				if (error.code !== "ESRCH") throw error;
			}
		}
		await closed;
		rmSync(caller, { recursive: true, force: true });
	});
	const [[status], stdout, stderr] = await Promise.all([closed, text(child.stdout), text(child.stderr)]);
	assert.equal(status, 23, stderr);
	assert.equal(readFileSync(defaultAlias, "utf8"), "login");
	const profile = stdout.trim();
	assert.ok(profile);
	assert.equal(existsSync(profile), false, "failed command must not leave its profile behind");
});

async function verifyCancellation(t, stopSupervisor) {
	const child = spawn("bash", [runner, "bash", "-euc", `
		bash -c 'trap "" TERM; printf "%s\\n" "$BASHPID" "$HOME"; exec sleep infinity' &
		wait "$!"
	`], { stdio: ["ignore", "pipe", "inherit"] });
	const exited = once(child, "exit");
	const output = createInterface({ input: child.stdout, signal: t.signal });
	let supervisor;
	t.after(() => {
		if (child.exitCode === null && child.signalCode === null) {
			if (supervisor !== undefined) {
				try { process.kill(supervisor, "SIGCONT"); } catch (error) {
					if (error.code !== "ESRCH") throw error;
				}
			}
			child.kill("SIGTERM");
		}
		output.close();
	});
	const lines = output[Symbol.asyncIterator]();
	const descendant = Number((await lines.next()).value);
	const profile = (await lines.next()).value;
	assert.ok(Number.isSafeInteger(descendant) && descendant > 0);
	try {
		assert.ok(profile);
		if (stopSupervisor) {
			const pid = Number(readFileSync(`/proc/${child.pid}/task/${child.pid}/children`, "utf8").trim());
			assert.ok(Number.isSafeInteger(pid) && pid > 0, "expected one session supervisor");
			process.kill(pid, "SIGSTOP");
			supervisor = pid;
		}
		assert.equal(child.kill("SIGTERM"), true);
		assert.deepEqual(await exited, [143, null]);
		assert.equal(existsSync(profile), false);
		for (;;) {
			let status;
			try { status = readFileSync(`/proc/${descendant}/stat`, "utf8"); } catch (error) {
				if (error.code === "ENOENT") break;
				throw error;
			}
			if (/\) [ZX] /.test(status)) break;
			await delay(1_000, undefined, { signal: t.signal });
		}
	} catch (error) {
		try { process.kill(descendant, "SIGKILL"); } catch (signalError) {
			if (signalError.code !== "ESRCH") throw signalError;
		}
		throw error;
	}
}

test("cancelling the desktop stops a descendant that ignores TERM", { skip: process.platform !== "linux", timeout: readinessTimeoutMs }, async (t) => {
	await verifyCancellation(t, false);
});

test("cancelling the desktop still cleans up when the supervisor is stopped", { skip: process.platform !== "linux", timeout: readinessTimeoutMs * 2 }, async (t) => {
	await verifyCancellation(t, true);
});
