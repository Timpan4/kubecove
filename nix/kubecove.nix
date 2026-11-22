{
  lib,
  rustPlatform,
  makeBinaryWrapper,
  pkg-config,
  fontconfig,
  libxcb,
  libxkbcommon,
  libglvnd,
  vulkan-loader,
  wayland,
  kubeconform,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "kubecove";
  version = (builtins.fromTOML (builtins.readFile ../rust/gpui/Cargo.toml)).package.version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.difference ../rust (lib.fileset.unions [
      (lib.fileset.maybeMissing ../rust/bin)
      (lib.fileset.maybeMissing ../rust/gpui/build)
      (lib.fileset.maybeMissing ../rust/gpui/target)
    ]);
  };

  cargoRoot = "rust/gpui";
  buildAndTestSubdir = finalAttrs.cargoRoot;
  cargoLock.lockFile = ../rust/gpui/Cargo.lock;
  cargoBuildFlags = [ "--bin" "kubecove-gpui" ];
  doCheck = false;

  nativeBuildInputs = [ makeBinaryWrapper pkg-config ];
  buildInputs = [ fontconfig libxcb libxkbcommon ];

  # GPUI loads the graphics and Wayland libraries at runtime.
  postInstall = ''
    mv "$out/bin/kubecove-gpui" "$out/bin/kubecove"
    wrapProgram "$out/bin/kubecove" \
      --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath [ fontconfig libglvnd vulkan-loader wayland ]}" \
      --set KUBECOVE_KUBECONFORM "${lib.getExe kubeconform}"
  '';

  meta = {
    description = "A context-first desktop workspace for Kubernetes operations";
    homepage = "https://github.com/Timpan4/kubecove";
    license = lib.licenses.agpl3Plus;
    mainProgram = "kubecove";
    platforms = lib.platforms.linux;
  };
})
