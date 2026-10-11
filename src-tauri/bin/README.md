Kubeconform sidecars pinned to v0.7.0.

Source: https://github.com/yannh/kubeconform/releases/tag/v0.7.0

Tauri bundles `bin/kubeconform` by selecting the file whose suffix matches the target triple.

Linux ARM64 uses `kubeconform-aarch64-unknown-linux-gnu`, downloaded from the
same v0.7.0 release as the existing sidecars. Its upstream archive SHA256 is
`cc907ccf9e3c34523f0f32b69745265e0a6908ca85b92f41931d4537860eb83c`, verified
against that release's `CHECKSUMS`. The binary is statically linked.
