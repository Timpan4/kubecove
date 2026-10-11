# Install the server

`kubecove-server` ships as native archives and as container images. Every option runs the same server with the same sign-in and safety rules. This page covers installing, upgrading and troubleshooting each one. For clients, sign-in modes and server options, see [Run KubeCove on a remote host](https://github.com/Timpan4/kubecove/wiki/Remote-Server).

## Choose an install method

| Method | Use it when | Platforms |
| --- | --- | --- |
| [Native archive with systemd](#native-archive-with-systemd) | A Linux host should run KubeCove as a service. Works with kubeconfig exec plugins such as `aws` or `gke-gcloud-auth-plugin` installed on the host. | Linux x64, Linux arm64 |
| [Native archive, run directly](#native-archive-run-directly) | You want a quick trial, or you run it under your own user on macOS. | Linux x64, Linux arm64, macOS arm64 |
| [Docker](#docker) | The host already runs containers, and the kubeconfig uses tokens or certificates rather than exec plugins. | linux/amd64, linux/arm64 |
| [Kubernetes](#kubernetes) | KubeCove should run inside a cluster and read it through a ServiceAccount. | linux/amd64, linux/arm64 |

## Choose full or slim

Each method comes in two variants with the same server options and authentication.

| Variant | Archive suffix | Image tag | Contents |
| --- | --- | --- | --- |
| Full | none | `ghcr.io/timpan4/kubecove:<version>` | Server, browser app and kubeconform |
| Slim | `-slim` | `ghcr.io/timpan4/kubecove:<version>-slim` | Server API and kubeconform, no browser app |

Choose full for browser use, including Cloudflare Access and OIDC. Choose slim when only the [desktop remote client](https://github.com/Timpan4/kubecove/wiki/Remote-Server#connect-from-the-desktop-app) connects. Slim serves no login page.

Linux arm64 archives, slim archives and arm64 images are published from the release after v0.12.0. Check the release assets for the version you install.

## Native archives

Release assets are named `kubecove-server_<version>_<target>[-slim].tar.gz`, each with a matching `.sha256` file. The targets are `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` and `aarch64-apple-darwin`.

Each archive contains:

- `kubecove-server`, the server executable;
- `kubeconform-<target>`, the YAML validation sidecar, which must stay in the same directory;
- `LICENSE`;
- `kubecove.service`, the systemd unit (Linux archives only).

Download the archive and its checksum, then verify before extracting:

```sh
sha256sum -c kubecove-server_<version>_<target>.tar.gz.sha256
```

On macOS, use `shasum -a 256 -c` instead.

The Linux executables need glibc. They do not need GTK, WebKit or a display.

### Native archive with systemd

The bundled unit runs the server as a dedicated `kubecove` user, keeps state in `/var/lib/kubecove` and reads its kubeconfig from `/etc/kubecove/config`. It listens on `127.0.0.1:8080` and expects browsers at `http://localhost:8080` through an SSH forward.

1. Create the service user. systemd creates its state directory on first start.

   ```sh
   sudo useradd --system --home-dir /var/lib/kubecove --shell /usr/sbin/nologin kubecove
   ```

2. Install the release into its own directory and point `current` at it:

   ```sh
   sudo install -d /opt/kubecove/releases/<version>
   sudo tar -xzf kubecove-server_<version>_<target>.tar.gz -C /opt/kubecove/releases/<version>
   sudo ln -sfn /opt/kubecove/releases/<version> /opt/kubecove/current
   ```

3. Install the kubeconfig the service should use. The service cannot read home directories, so the file must live outside them:

   ```sh
   sudo install -d -m 0750 -o root -g kubecove /etc/kubecove
   sudo install -m 0640 -o root -g kubecove /path/to/kubeconfig /etc/kubecove/config
   ```

   Exec credential plugins must be installed system-wide, for example in `/usr/local/bin`. Their caches go to `/var/lib/kubecove`.

4. Install and start the unit:

   ```sh
   sudo install -m 0644 /opt/kubecove/current/kubecove.service /etc/systemd/system/kubecove.service
   sudo systemctl daemon-reload
   sudo systemctl enable --now kubecove
   ```

   To change the port or public origin, run `sudo systemctl edit kubecove` and override `ExecStart`. Keep `--quiet` and `--data-dir /var/lib/kubecove`.

5. Read the token for this start. It changes on every restart:

   ```sh
   sudo cat /var/lib/kubecove/server-token
   ```

6. From the laptop, forward the port and open `http://localhost:8080`:

   ```sh
   ssh -N -L 8080:127.0.0.1:8080 user@remote-host
   ```

The unit discards stdout so a login URL never reaches the journal. Errors still go to `journalctl -u kubecove`. It also applies systemd sandboxing: no capabilities, a read-only system, hidden home directories, private `/tmp` and devices, and a restricted system call set. `MemoryDenyWriteExecute` is left off, because it breaks exec plugins that use a JIT runtime.

**Upgrade.** Extract the new version into its own `releases/<version>` directory, move the `current` link, and restart:

```sh
sudo ln -sfn /opt/kubecove/releases/<new-version> /opt/kubecove/current
sudo systemctl restart kubecove
```

**Roll back.** Point `current` at the previous release and restart. Data in `/var/lib/kubecove` stays in place either way. If a release changes the unit, reinstall it from the new archive.

### Native archive, run directly

Run the server as the user whose kubeconfig and cluster access KubeCove should use:

```sh
tar -xzf kubecove-server_<version>_<target>.tar.gz
./kubecove-server --listen 127.0.0.1:8080 --public-origin http://localhost:8080
```

Startup prints a login URL containing a fresh token. Treat it as a password. The token is also stored in `server-token` in the data directory; run `./kubecove-server --help` for `--data-dir` and the other options. On macOS, Gatekeeper may block an unsigned download. Allow it in System Settings → Privacy & Security, or remove the quarantine attribute from the extracted files.

To upgrade, stop the server, replace both executables with the new release, and start it again.

## Container images

Images are built for `linux/amd64` and `linux/arm64` on a [distroless](https://github.com/GoogleContainerTools/distroless) base. They contain the server, glibc, CA certificates and kubeconform, and run as UID and GID `65534`. They have no shell or package manager, so `docker exec ... sh` does not work. The image's default command adds `--quiet`, so container logs never contain a login URL.

Images do not include cloud CLIs or other exec credential plugins. A kubeconfig that runs `aws`, `gke-gcloud-auth-plugin`, `kubelogin` or similar fails inside the container. Use a native install for those kubeconfigs, or a kubeconfig with a token or client certificate.

### Docker

On a Linux Docker host, host networking keeps the listener on the host's loopback, so the SSH forward works unchanged. Run the container as your own user so it can read your kubeconfig and write a private data directory:

```sh
install -d -m 0700 ~/.local/share/kubecove
docker run -d --name kubecove --restart unless-stopped \
  --network host --user "$(id -u):$(id -g)" \
  --read-only --tmpfs /tmp --cap-drop ALL --security-opt no-new-privileges \
  -v ~/.local/share/kubecove:/data/kubecove \
  -v ~/.kube/config:/etc/kubecove/config:ro -e KUBECONFIG=/etc/kubecove/config \
  ghcr.io/timpan4/kubecove:<version> \
  --quiet --listen 127.0.0.1:8080 --public-origin http://localhost:8080 --data-dir /data/kubecove
```

Read the token on the host:

```sh
cat ~/.local/share/kubecove/server-token
```

Then forward the port from the laptop as in the systemd steps. Pin the image by digest for repeatable deployments.

To upgrade, pull the new tag, then remove and re-create the container with the same arguments. The data directory carries over.

To expose the server beyond loopback instead, use `--allow-remote` with `--tls-cert`, `--tls-key` and an exact HTTPS `--public-origin`. See [security and support boundaries](https://github.com/Timpan4/kubecove/wiki/Remote-Server#security-and-support-boundaries).

### Kubernetes

The [Kubernetes example](https://github.com/Timpan4/kubecove/blob/main/deploy/kubernetes/kubecove.yaml) runs one replica with a ServiceAccount bound to the built-in `view` role. The [deployment guide](https://github.com/Timpan4/kubecove/blob/main/deploy/README.md) covers storage, RBAC choices, Cloudflare Tunnel and OIDC.

The image has no shell, so read the token through a temporary debug container that mounts the data volume read-only:

```sh
pod="$(kubectl -n kubecove get pod -l app.kubernetes.io/name=kubecove -o jsonpath='{.items[0].metadata.name}')"
kubectl -n kubecove debug "$pod" --quiet --attach --container token-reader \
  --image busybox:1.37.0-musl@sha256:5cec3fc171c87218698e85a52af7087de727372aae264a787b8112901a5b0092 \
  --custom deploy/kubernetes/token-reader.json -- cat /data/kubecove/server-token
```

This needs permission to create ephemeral containers (`pods/ephemeralcontainers`). Each read leaves a stopped ephemeral container in the pod until the pod is replaced. Use a new `--container` name for each read.

To upgrade, change the image reference and apply the manifest. The `Recreate` strategy stops the old pod before the new one starts.

## Troubleshoot without a shell

- **Check the binary.** `docker run --rm ghcr.io/timpan4/kubecove:<version> --help`.
- **See errors.** Errors go to stderr: `docker logs kubecove` or `kubectl -n kubecove logs deployment/kubecove`. Empty logs on a healthy start are expected.
- **Inspect files.** Read the host-side data directory with Docker. Use a debug container as above in Kubernetes.
- **Need host tools.** Use a native archive, which runs with the host's normal tools.

For sign-in problems, see [Troubleshooting](https://github.com/Timpan4/kubecove/wiki/Troubleshooting).
