import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const runner = fileURLToPath(new URL("./headless-desktop.sh", import.meta.url));

test("headless desktop changes only its own Secret Service default", { skip: process.platform !== "linux" }, (t) => {
	const caller = mkdtempSync(join(tmpdir(), "kubecove-keyring-caller-"));
	t.after(() => rmSync(caller, { recursive: true, force: true }));
	const data = join(caller, "data");
	const keyrings = join(data, "keyrings");
	mkdirSync(keyrings, { recursive: true });
	const defaultAlias = join(keyrings, "default");
	writeFileSync(defaultAlias, "caller-keyring");

	const result = spawnSync("bash", [runner, "bash", "-euc", `
		test "$HOME" != "$CALLER_HOME"
		for directory in "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_STATE_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR"; do
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
			XDG_RUNTIME_DIR: join(caller, "runtime"),
			CALLER_BUS: `unix:path=${join(caller, "caller-bus")}`,
			DBUS_SESSION_BUS_ADDRESS: `unix:path=${join(caller, "caller-bus")}`,
			DBUS_STARTER_ADDRESS: `unix:path=${join(caller, "caller-bus")}`,
			GNOME_KEYRING_CONTROL: join(caller, "caller-keyring"),
			WAYLAND_DISPLAY: "caller-wayland",
			SSH_AUTH_SOCK: join(caller, "caller-ssh"),
		},
		encoding: "utf8",
	});
	assert.equal(readFileSync(defaultAlias, "utf8"), "caller-keyring");
	assert.equal(result.status, 23, result.stderr);
	const profile = result.stdout.trim();
	assert.ok(profile);
	assert.equal(existsSync(profile), false, "failed command must not leave its profile behind");
});
