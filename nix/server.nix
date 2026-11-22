{
  lib,
  rustPlatform,
  makeBinaryWrapper,
  kubeconform,
  frontend,
}:

let
  version = (builtins.fromTOML (builtins.readFile ../rust/server/Cargo.toml)).package.version;
in
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "kubecove-server";
  inherit version;

  # The vendored kubeconform binaries stay out: the wrapper points at
  # nixpkgs' kubeconform instead.
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.difference ../rust ../rust/bin;
  };

  cargoRoot = "rust/server";
  buildAndTestSubdir = finalAttrs.cargoRoot;
  cargoLock.lockFile = ../rust/server/Cargo.lock;
  doCheck = false;

  nativeBuildInputs = [ makeBinaryWrapper ];

  # rust-embed reads `../../dist/` relative to the server crate.
  preBuild = ''
    cp -r ${frontend} dist
  '';

  postInstall = ''
    wrapProgram $out/bin/kubecove-server \
      --set-default KUBECOVE_KUBECONFORM ${lib.getExe kubeconform}
  '';

  meta = {
    description = "Authenticated single-user headless KubeCove host";
    homepage = "https://github.com/Timpan4/kubecove";
    license = lib.licenses.agpl3Plus;
    mainProgram = "kubecove-server";
    platforms = lib.platforms.linux;
  };
})
