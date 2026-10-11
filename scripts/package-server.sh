#!/usr/bin/env bash
# Package the actual native build, including its matching validation sidecar.
set -euo pipefail
target="${1:?native target required}"
variant="${2:?full or slim required}"
version="${3:?release version required}"
case "$target" in
  x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu|aarch64-apple-darwin) ;;
  *) exit 1 ;;
esac
case "$variant" in full) suffix="" ;; slim) suffix="-slim" ;; *) exit 1 ;; esac
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]
binary="src-tauri/server/target/$target/release/kubecove-server"
sidecar="kubeconform-$target"
temporary="$(mktemp -d)"
trap 'rm -rf "$temporary"' EXIT
install -m 0755 "$binary" "$temporary/kubecove-server"
install -m 0755 "src-tauri/bin/$sidecar" "$temporary/$sidecar"
install -m 0644 LICENSE "$temporary/LICENSE"
if [[ "$target" == *-linux-gnu ]]; then
  install -m 0644 deploy/systemd/kubecove.service "$temporary/kubecove.service"
fi
# Each release job runs on this target, so execute the packaged files too.
"$temporary/kubecove-server" --help >/dev/null
"$temporary/$sidecar" -v >/dev/null
archive="kubecove-server_${version}_${target}${suffix}.tar.gz"
# mktemp creates 0700; the extracted root must stay traversable for the service user.
chmod 0755 "$temporary"
if tar --version | grep -q 'GNU tar'; then
  tar -czf "$archive" --owner=0 --group=0 --numeric-owner -C "$temporary" .
else
  tar -czf "$archive" --uid 0 --gid 0 --numeric-owner -C "$temporary" .
fi
# shasum ships on both Linux runners and macOS; its output works with sha256sum -c.
shasum -a 256 "$archive" > "$archive.sha256"
printf 'Packaged %s\n' "$archive"
