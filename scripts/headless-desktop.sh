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
export TMPDIR="$profile/tmp"
unset DBUS_SESSION_BUS_ADDRESS DBUS_STARTER_ADDRESS DBUS_STARTER_BUS_TYPE GNOME_KEYRING_CONTROL
unset BASH_ENV ENV SSH_AUTH_SOCK WAYLAND_DISPLAY WAYLAND_SOCKET DISPLAY XAUTHORITY
mkdir -m 700 "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME" "$XDG_STATE_HOME" "$XDG_RUNTIME_DIR" "$TMPDIR"
mkfifo "$profile/shutdown"
exec {shutdown_fd}<>"$profile/shutdown"

stop() {
    trap '' INT TERM
    # Match the E2E harness shutdown budget. The timer lives outside the session it stops.
    (
        if ! read -r -t 30 -u "$shutdown_fd"; then
            kill -KILL -- "-$worker" 2>/dev/null || true
        fi
    ) &
    local watchdog=$!
    kill -TERM -- "-$worker" 2>/dev/null || kill -TERM "$worker" 2>/dev/null || true
    wait "$worker" 2>/dev/null || true
    printf '\n' >&"$shutdown_fd"
    wait "$watchdog"
    exit "$1"
}
setsid bash -euc '
    stop_session() {
        trap "" TERM
        if [[ -n ${session:-} ]]; then wait "$session" || true; fi
        # Xvfb removes its display socket and lock when it handles TERM.
        while ps -s "$$" -o comm=,stat= | grep -c "^Xvfb[[:blank:]]\+[^ZX]" >/dev/null; do sleep 1; done
        kill -KILL -- "-$$"
    }
    trap stop_session TERM
    dbus-run-session -- bash -euc "$1" headless-desktop "${@:2}" &
    session=$!
    wait "$session"
' headless-desktop-session '
    dbus-send --session --print-reply --dest=org.freedesktop.secrets \
        /org/freedesktop/secrets org.freedesktop.Secret.Service.SetAlias \
        string:default objpath:/org/freedesktop/secrets/collection/session >/dev/null
    exec xvfb-run -a "$@"
' "$@" {shutdown_fd}>&- &
worker=$!
trap 'stop 130' INT
trap 'stop 143' TERM
wait "$worker"
