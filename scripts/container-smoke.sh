#!/usr/bin/env bash
# Run on a Linux Docker host. No network, cluster or startup credential output.
set -euo pipefail
umask 077
image="${1:?image required}"
variant="${2:?full or slim required}"
case "$variant" in full) root_status=200 ;; slim) root_status=404 ;; *) exit 1 ;; esac
temporary="$(mktemp -d)"
container=""
cleanup() {
  if [[ -n "$container" ]]; then docker rm -f "$container" >/dev/null; fi
  rm -rf "$temporary"
}
trap cleanup EXIT
test "$(docker image inspect --format '{{.Config.User}}' "$image")" = '65534:65534'
chown 65534:65534 "$temporary"
chmod 700 "$temporary"
container="$(docker run -d --network none --read-only --cap-drop ALL \
  --security-opt no-new-privileges --mount "type=bind,src=$temporary,dst=/data/kubecove" \
  "$image" --quiet --socket /data/kubecove/check.sock --public-origin http://localhost:8080 \
  --data-dir /data/kubecove)"
while [[ ! -S "$temporary/check.sock" ]]; do
  test "$(docker inspect --format '{{.State.Running}}' "$container")" = true
  sleep 1
done
curl --silent --show-error \
  --unix-socket "$temporary/check.sock" http://localhost:8080/api/v1/auth > "$temporary/auth.json"
grep -q '"mode":"token"' "$temporary/auth.json"
status() {
  curl --silent --show-error --unix-socket "$temporary/check.sock" \
    --output /dev/null --write-out '%{http_code}' "$@"
}
test "$(status http://localhost:8080/)" = "$root_status"
test "$(status -H 'Host: untrusted.example' http://localhost:8080/)" = 403
test "$(status -X POST -H 'Origin: http://localhost:8080' -H 'Content-Type: application/json' \
  -d '{}' http://localhost:8080/api/v1/session)" = 401
test "$(stat -c %u "$temporary/server-token")" = 65534
test "$(stat -c %a "$temporary/server-token")" = 600
docker cp "$container:/etc/ssl/certs/ca-certificates.crt" "$temporary/ca.crt"
test -s "$temporary/ca.crt"
docker exec "$container" /usr/local/bin/kubeconform -v >/dev/null
docker export "$container" | tar -tf - > "$temporary/runtime-files.txt"
! grep -Eq '(^|/)(sh|bash|cargo|bun|apt|apt-get)$' "$temporary/runtime-files.txt"
# Feed credentials through stdin, never through command arguments or logs.
cat "$temporary/server-token" | \
  { read -r token || test -n "$token"; printf '{"token":"%s"}' "$token"; } > "$temporary/login.json"
test "$(status -X POST -H 'Origin: http://localhost:8080' -H 'Content-Type: application/json' \
  --data-binary @"$temporary/login.json" http://localhost:8080/api/v1/session)" = 200
docker stop "$container" >/dev/null
test "$(docker inspect --format '{{.State.ExitCode}}' "$container")" = 0
docker logs "$container" > "$temporary/stdout.txt"
test ! -s "$temporary/stdout.txt"
printf '%s image: distroless, non-root, read-only runtime, CA, sidecar, UI selection, authentication, quiet startup and shutdown passed\n' "$variant"
