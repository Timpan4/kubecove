#!/bin/sh
# Builds the GPUI client in release mode and opens it on the user's desktop for hands-on
# use. The launch runs as a systemd user unit so it outlives the agent session that starts
# it. Only run this when the user asks for a native window.
# Usage: scripts/gpui-feel.sh   (from any directory inside the checkout)
set -eu

root=$(git rev-parse --show-toplevel)
gpui="$root/src-tauri/gpui"
feel="$gpui/target/feel"
mkdir -p "$feel"

# Hold a launch lock from the unit check through writing unit.txt, so two runs cannot both
# pass the check and open two windows from one binary copy.
exec 9>"$feel/launch.lock"
if ! flock -n 9; then
  echo "Another gpui-feel.sh run is building or launching in this checkout."
  exit 1
fi

# Reuse a window this script already opened instead of starting a second one.
if [ -f "$feel/unit.txt" ] && systemctl --user is-active --quiet "$(cat "$feel/unit.txt")"; then
  echo "KubeCove GPUI is already running as $(cat "$feel/unit.txt"). Stop that unit first to relaunch."
  exit 1
fi

# Per-worktree build directories, so concurrent checkouts do not overwrite each other's crates.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$gpui/target}"
export CARGO_BUILD_BUILD_DIR="${CARGO_BUILD_BUILD_DIR:-$gpui/build}"
export RUSTC_WRAPPER=
echo "Building release binary; log: $feel/build.log"
# 9>&- keeps the launch lock out of the build, whose helper processes may outlive it.
flock "$gpui/target/heavy.lock" nice -n 10 \
  cargo build --manifest-path "$gpui/Cargo.toml" --release --bin kubecove-gpui >"$feel/build.log" 2>&1 9>&-

# Run a copy so a later build can replace the release binary while this window stays open.
cp "$CARGO_TARGET_DIR/release/kubecove-gpui" "$feel/kubecove-gpui"
sha256sum "$feel/kubecove-gpui" >"$feel/binary.sha256"

unit="kubecove-gpui-feel-$(date +%s)"
# The user manager does not inherit this shell's PATH or display variables, so pass them.
systemd-run --user --collect --unit="$unit" --working-directory="$root" \
  --setenv=PATH="$PATH" \
  --setenv=HOME="$HOME" \
  --setenv=XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}" \
  ${WAYLAND_DISPLAY:+--setenv=WAYLAND_DISPLAY="$WAYLAND_DISPLAY"} \
  ${DISPLAY:+--setenv=DISPLAY="$DISPLAY"} \
  "$feel/kubecove-gpui"
echo "$unit" >"$feel/unit.txt"
echo "Started $unit from $(git rev-parse --short HEAD). Logs: journalctl --user -u $unit"
