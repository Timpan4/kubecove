#!/usr/bin/env bash
set -euo pipefail

if [[ $# -eq 0 ]]; then
    printf 'Usage: bash scripts/headless-desktop.sh COMMAND [ARG...]\n' >&2
    exit 2
fi

# Keep the existing toolchain while replacing the desktop's home and state.
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
temp_base=$(cd -- "${TMPDIR:-/tmp}" && pwd -P)
profile=$(mktemp -d "$temp_base/kubecove-desktop.XXXXXXXX")
trap 'rm -rf -- "$profile"' EXIT
export HOME="$profile"
export XDG_CONFIG_HOME="$profile/config" XDG_DATA_HOME="$profile/data"
export XDG_CACHE_HOME="$profile/cache" XDG_STATE_HOME="$profile/state" XDG_RUNTIME_DIR="$profile/runtime"
unset DBUS_SESSION_BUS_ADDRESS DBUS_STARTER_ADDRESS DBUS_STARTER_BUS_TYPE GNOME_KEYRING_CONTROL
unset BASH_ENV ENV SSH_AUTH_SOCK WAYLAND_DISPLAY WAYLAND_SOCKET DISPLAY XAUTHORITY
mkdir -m 700 "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME" "$XDG_STATE_HOME" "$XDG_RUNTIME_DIR"

stop() {
    kill -TERM -- "-$worker" 2>/dev/null || true
    wait "$worker" || true
    exit "$1"
}
setsid dbus-run-session -- bash -euc '
    gnome-keyring-daemon --start --components=secrets >/dev/null
    dbus-send --session --print-reply --dest=org.freedesktop.secrets \
        /org/freedesktop/secrets org.freedesktop.Secret.Service.SetAlias \
        string:default objpath:/org/freedesktop/secrets/collection/session >/dev/null
    exec xvfb-run -a "$@"
' headless-desktop "$@" &
worker=$!
trap 'stop 130' INT
trap 'stop 143' TERM
wait "$worker"
