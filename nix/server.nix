{
  lib,
  rustPlatform,
  makeBinaryWrapper,
  kubeconform,
  frontend,
}:

let
  version = (builtins.fromTOML (builtins.readFile ../src-tauri/server/Cargo.toml)).package.version;
in
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "kubecove-server";
  inherit version;

  # The vendored kubeconform binaries stay out: the wrapper points at
  # nixpkgs' kubeconform instead.
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.difference ../src-tauri ../src-tauri/bin;
  };

  cargoRoot = "src-tauri/server";
  buildAndTestSubdir = finalAttrs.cargoRoot;
  cargoHash = "sha256-xgESFwLKcBD/lA+zJ3JS/bqCcMRSTOQpHNOoqJJ5T88=";
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
